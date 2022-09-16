use instant::Instant;
use nalgebra_glm as glm;
use std::rc::Rc;
use std::time::Duration;
use winit::dpi::PhysicalPosition;
use winit::event::DeviceEvent;
use winit::event::ElementState;
use winit::event::Event;
use winit::event::Touch;
use winit::event::WindowEvent;
use winit::event_loop::ControlFlow;
use winit::window::Window;

use crate::ai;
use crate::game;
use crate::graphics;

type WinitEvent<'a> = Event<'a, ()>;

pub struct GameLoop {
    window: Window,
    level: game::Level,
    game: game::Game,
    gfx: graphics::Instance,
    ui: Ui,
    scene: graphics::Scene,
    board_node_id: graphics::NodeId,
    ball_node_id: graphics::NodeId,
    static_camera: bool,
    state: State,
    last_cursor_pos: Option<PhysicalPosition<f64>>,
    double_tap_start_t: Option<Instant>,
    last_touch_pos: Option<PhysicalPosition<f64>>,
    timer: Stopwatch,
    stats: Option<Stats>,
    ai: Option<Box<dyn ai::GameAi>>,
}

impl GameLoop {
    pub fn new(
        window: Window,
        level: game::Level,
        gfx: graphics::Instance,
        width_pixels: u32,
        height_pixels: u32,
        scene: graphics::Scene,
        board_node_id: graphics::NodeId,
        ball_node_id: graphics::NodeId,
        static_camera: bool,
        print_stats: bool,
        mut ai: Option<Box<dyn ai::GameAi>>,
    ) -> GameLoop {
        let game = game::Game::new(&level);
        if let Some(ai) = &mut ai {
            ai.init(&level);
        }
        GameLoop {
            window,
            level,
            game,
            gfx,
            ui: Ui::new(
                width_pixels,
                height_pixels,
                width_pixels as f32 / 800.0, // Scale the UI to always take the same relative amount of the available space
            ),
            scene,
            board_node_id,
            ball_node_id,
            static_camera,
            state: State::GameInProgress,
            last_cursor_pos: None,
            double_tap_start_t: None,
            last_touch_pos: None,
            timer: Stopwatch::start_new(),
            stats: if print_stats {
                Some(Stats {
                    frame_count: 0,
                    last_calculated_t: Instant::now(),
                })
            } else {
                None
            },
            ai,
        }
    }

    pub fn handle_event(&mut self, event: &WinitEvent) -> ControlFlow {
        match event {
            Event::Suspended => self.suspended(),
            Event::Resumed => self.resumed(),
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => return ControlFlow::Exit,
                WindowEvent::CursorMoved { position, .. } => self.cursor_moved(position),
                WindowEvent::MouseInput { state, .. } => self.mouse_click(state),
                WindowEvent::Touch(touch) => self.touch(touch),
                _ => (),
            },
            Event::DeviceEvent {
                event: DeviceEvent::MouseMotion { delta },
                ..
            } => self.mouse_moved(delta),
            Event::MainEventsCleared => {
                if !self.do_frame() {
                    return ControlFlow::Exit;
                }
            }
            _ => (),
        }

        return match self.state {
            State::GameInProgress => ControlFlow::Poll,
            State::GamePaused => ControlFlow::Wait,
        };
    }

    fn do_frame(&mut self) -> bool {
        match self.state {
            State::GameInProgress => {
                let now = Instant::now();
                if let Some(ai) = &mut self.ai {
                    let next_move = ai.next_move(&self.game, now);
                    self.game.rotate_x(next_move.x);
                    self.game.rotate_y(next_move.y);
                }
                self.update_frame_stats(now);
                let ball_pos_delta = self.update_game(now);
                if !self.update_scene(now, ball_pos_delta) {
                    // Scene is not alive anymore, meaning the game has been won/lost.
                    // Pause the game to stop the timer and show the menu.
                    self.pause_game();
                }
            }
            State::GamePaused => (),
        }
        let ui_output =
            self.ui
                .update(&self.gfx, self.timer.elapsed(), self.state, self.game.state);
        self.gfx.render_scene(&self.scene, &ui_output.objects);
        for action in ui_output.actions.iter() {
            match action {
                UiAction::ResumeGame => self.resume_game(),
                UiAction::RestartLevel => self.restart_level(),
                UiAction::Quit => return false,
            }
        }
        return true;
    }

    fn suspended(&mut self) {
        // When the application is suspended (on mobile platforms), the window object becomes
        // unusable, so reset the handle used in graphics code.
        self.gfx.set_window(None as Option<&winit::window::Window>);
        match self.state {
            State::GameInProgress => self.pause_game(),
            State::GamePaused => (),
        }
    }

    fn resumed(&mut self) {
        self.gfx.set_window(Some(&self.window));
    }

    fn mouse_click(&mut self, state: &ElementState) {
        match self.state {
            State::GameInProgress => match state {
                ElementState::Pressed => self.pause_game(),
                ElementState::Released => (),
            },
            State::GamePaused => {
                if let Some(pos) = self.last_cursor_pos {
                    self.ui.click(
                        pos.x as f32,
                        pos.y as f32,
                        match state {
                            ElementState::Pressed => true,
                            ElementState::Released => false,
                        },
                    );
                }
            }
        }
    }

    // Any mouse movement -> control game
    fn mouse_moved(&mut self, delta: &(f64, f64)) {
        match self.state {
            State::GameInProgress => {
                const ROTATE_COEFF: f32 = 0.0002;
                if self.ai.is_none() {
                    self.game.rotate_x(ROTATE_COEFF * delta.0 as f32);
                    self.game.rotate_y(ROTATE_COEFF * delta.1 as f32);
                }
            }
            State::GamePaused => (),
        }
    }

    // New cursor position in the window -> control UI
    fn cursor_moved(&mut self, pos: &PhysicalPosition<f64>) {
        self.last_cursor_pos = Some(*pos);
        match self.state {
            State::GameInProgress => (),
            State::GamePaused => self.ui.cursor_moved(pos.x as f32, pos.y as f32),
        }
    }

    fn touch(&mut self, touch: &Touch) {
        match touch.phase {
            winit::event::TouchPhase::Started => self.touch_started(touch.location),
            winit::event::TouchPhase::Moved => self.swipe(touch.location),
            winit::event::TouchPhase::Ended => self.touch_ended(touch.location),
            _ => (),
        }
    }

    fn update_game(&mut self, now: Instant) -> glm::Vec3 {
        let p0 = self.game.ball_pos;
        self.game.update(now);
        return glm::vec3(
            self.game.ball_pos.x - p0.x,
            0.0,
            self.game.ball_pos.y - p0.y,
        );
    }

    // Return true if the scene is still alive, false if it has reached a static state
    fn update_scene(&mut self, now: Instant, ball_pos_delta: glm::Vec3) -> bool {
        match self.game.state {
            game::State::InProgress => {
                self.update_board();
                let ball_pos = self.ball_pos_in_scene();
                self.update_ball(ball_pos, ball_pos_delta);
                self.update_camera(ball_pos);
                true
            }
            game::State::Lost { hole, t_lost } => {
                self.update_ball_game_lost(now, t_lost, hole)
                // Keep the scene alive for some minimum time after game is lost even if the animation finishes faster
                    || now.duration_since(t_lost).as_secs_f32() < 0.5
            }
            game::State::Won => false,
        }
    }

    fn update_frame_stats(&mut self, now: Instant) {
        match &mut self.stats {
            Some(s) => {
                s.frame_count = s.frame_count + 1;
                let elapsed = now.duration_since(s.last_calculated_t);
                if elapsed >= Duration::from_secs(5) {
                    println!("FPS {}", s.frame_count as f64 / elapsed.as_secs_f64());
                    s.frame_count = 0;
                    s.last_calculated_t = now;
                }
            }
            None => (),
        }
    }

    fn update_board(&mut self) {
        self.scene.get_node(self.board_node_id).set_rotation(
            self.game.angle_y,
            0.0,
            -self.game.angle_x,
        );
    }

    fn ball_pos_in_scene(&self) -> glm::Vec3 {
        glm::vec3(
            self.game.ball_pos.x - self.game.level.size.w / 2.0,
            game::BALL_R,
            self.game.ball_pos.y - self.game.level.size.h / 2.0,
        )
    }

    fn update_ball(&mut self, ball_pos: glm::Vec3, ball_pos_delta: glm::Vec3) {
        self.scene
            .get_node(self.ball_node_id)
            .set_position(ball_pos.x, ball_pos.y, ball_pos.z);
        let axis_world_space = glm::normalize(&glm::rotate_vec3(
            &ball_pos_delta,
            std::f32::consts::PI / 2.0,
            &glm::vec3(0.0, 1.0, 0.0),
        ));
        self.scene
            .get_node(self.ball_node_id)
            .rotate_in_world_space(
                glm::length(&ball_pos_delta) / game::BALL_R,
                axis_world_space.x,
                axis_world_space.y,
                axis_world_space.z,
            );
    }

    fn update_camera(&mut self, ball_pos: glm::Vec3) {
        if !self.static_camera {
            self.scene.look_at(
                ball_pos.x,
                40.0 * game::BALL_R,
                ball_pos.z + 10.0 * game::BALL_R,
                ball_pos.x,
                0.0,
                ball_pos.z,
            );
        }
    }

    // Return true if the animation is in progress, false if it's done
    fn update_ball_game_lost(
        &mut self,
        now: Instant,
        t_lost: Instant,
        hole_pos: game::Point,
    ) -> bool {
        match animate_ball_falling_in_hole(
            now.duration_since(t_lost).as_secs_f32(),
            self.game.ball_pos,
            hole_pos,
        ) {
            Some((x, y, z)) => {
                self.scene.get_node(self.ball_node_id).set_position(
                    x - self.game.level.size.w / 2.0,
                    z,
                    y - self.game.level.size.h / 2.0,
                );
                true
            }
            None => {
                // Hide the ball after the animation is done
                self.scene.get_node(self.ball_node_id).set_position(
                    10.0 * self.game.level.size.w,
                    0.0,
                    0.0,
                );
                false
            }
        }
    }

    fn touch_started(&mut self, pos: PhysicalPosition<f64>) {
        match self.state {
            State::GameInProgress => {
                let now = Instant::now();
                self.double_tap_start_t = match self.double_tap_start_t {
                    Some(t0) if now.duration_since(t0) < Duration::from_millis(400) => {
                        self.double_tap();
                        None
                    }
                    _ => Some(now),
                };
                self.last_touch_pos = Some(pos);
            }
            State::GamePaused => self.ui.click(pos.x as f32, pos.y as f32, true),
        }
    }

    fn swipe(&mut self, pos: PhysicalPosition<f64>) {
        match self.state {
            State::GameInProgress => {
                if let Some(p0) = self.last_touch_pos {
                    const ROTATE_COEFF: f32 = 0.0004;
                    if self.ai.is_none() {
                        self.game.rotate_x(ROTATE_COEFF * (pos.x - p0.x) as f32);
                        self.game.rotate_y(ROTATE_COEFF * (pos.y - p0.y) as f32);
                    }
                }
                self.last_touch_pos = Some(pos);
            }
            State::GamePaused => self.ui.cursor_moved(pos.x as f32, pos.y as f32),
        }
    }

    fn touch_ended(&mut self, pos: PhysicalPosition<f64>) {
        match self.state {
            State::GameInProgress => (),
            State::GamePaused => self.ui.click(pos.x as f32, pos.y as f32, false),
        }
    }

    fn double_tap(&mut self) {
        match self.state {
            State::GameInProgress => self.pause_game(),
            State::GamePaused => (),
        }
    }

    fn pause_game(&mut self) {
        println!("Pausing game");
        self.state = State::GamePaused;
        self.game.reset_time();
        self.timer.stop();
        self.double_tap_start_t = None;
        self.last_touch_pos = None;
        if let Some(ai) = &mut self.ai {
            ai.pause();
        }
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            self.window.set_cursor_visible(true);
            self.window.set_cursor_grab(false).unwrap();
        }
    }

    fn resume_game(&mut self) {
        self.state = State::GameInProgress;
        self.timer.start();
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            self.window.set_cursor_visible(false);
            self.window.set_cursor_grab(true).unwrap();
        }
    }

    fn restart_level(&mut self) {
        self.game = game::Game::new(&self.level);
        self.timer = Stopwatch::start_new();
        if let Some(ai) = &mut self.ai {
            ai.init(&self.level);
        }
        self.resume_game(); // Ensure the game is in progress
    }
}

// Calculates the ball position (x, y, z) when the game has been lost and the ball is falling in to hole.
// x and y are in game coordinates, z is the vertical distance from the game's board surface.
// The animation has finite duration and `None` is returned when the animation has finished.
// `t` is the duration since (in s), and `last_ball_pos` is the ball position when the game was lost.
// `hole_pos` is the center of the hole where the ball is falling.
fn animate_ball_falling_in_hole(
    t: f32,
    last_ball_pos: game::Point,
    hole_pos: game::Point,
) -> Option<(f32, f32, f32)> {
    const ROLL_OVER_DURATION: f32 = 0.1;
    const FREE_FALL_DURATION: f32 = 0.1;
    const TOTAL_DURATION: f32 = ROLL_OVER_DURATION + FREE_FALL_DURATION;
    const FREE_FALL_DEPTH: f32 = 3.0 * game::BALL_R;

    let hole = glm::vec2(hole_pos.x, hole_pos.y);
    let ball0 = glm::vec2(last_ball_pos.x, last_ball_pos.y);

    // xy-point where the ball has completely rolled over the hole edge
    let free_fall_point = hole + glm::normalize(&(ball0 - hole)) * (game::HOLE_R - game::BALL_R);

    match t {
        t if t < ROLL_OVER_DURATION => {
            let xy = ball0 + (free_fall_point - ball0) * (t / ROLL_OVER_DURATION);
            let ds_hole_edge = game::HOLE_R - glm::distance(&hole, &xy);
            Some((
                xy.x,
                xy.y,
                (game::BALL_R.powi(2) - ds_hole_edge.powi(2)).sqrt(),
            ))
        }
        t if t < TOTAL_DURATION => Some((
            free_fall_point.x,
            free_fall_point.y,
            -(t - ROLL_OVER_DURATION) / FREE_FALL_DURATION * FREE_FALL_DEPTH,
        )),
        _ => None,
    }
}

#[derive(Copy, Clone, Debug)]
enum State {
    GameInProgress,
    GamePaused,
}

struct Stats {
    frame_count: u32,
    last_calculated_t: Instant,
}

struct Stopwatch {
    elapsed: Duration,
    start_t: Option<Instant>,
}

impl Stopwatch {
    fn start_new() -> Stopwatch {
        Stopwatch {
            elapsed: Duration::from_secs(0),
            start_t: Some(Instant::now()),
        }
    }
    fn start(&mut self) {
        if self.start_t.is_none() {
            self.start_t = Some(Instant::now());
        }
    }

    fn stop(&mut self) {
        if let Some(t0) = self.start_t {
            self.elapsed += Instant::now() - t0;
        }
        self.start_t = None;
    }

    fn elapsed(&self) -> Duration {
        match self.start_t {
            Some(t0) => self.elapsed + (Instant::now() - t0),
            None => self.elapsed,
        }
    }
}

struct Ui {
    ctx: egui::CtxRef,
    texture: Option<EguiTexture>,
    width_points: f32,
    height_points: f32,
    scale: f32,
    events: Vec<egui::Event>,
}

impl Ui {
    fn new(width_pixels: u32, height_pixels: u32, scale: f32) -> Ui {
        Ui {
            ctx: egui::CtxRef::default(),
            texture: None,
            width_points: width_pixels as f32 / scale,
            height_points: height_pixels as f32 / scale,
            scale,
            events: Vec::new(),
        }
    }

    fn update(
        &mut self,
        gfx: &graphics::Instance,
        elapsed: Duration,
        pause_state: State,
        game_state: game::State,
    ) -> UiOutput {
        let mut actions = Vec::new();
        self.ctx.begin_frame(egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::Vec2::new(self.width_points, self.height_points),
            )),
            pixels_per_point: Some(self.scale),
            events: std::mem::take(&mut self.events),
            ..Default::default()
        });
        egui::Window::new("Timer window")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .fixed_pos(egui::pos2(10.0, 10.0))
            .show(&self.ctx, |ui| {
                ui.label(format!(
                    "{:02}:{:02}",
                    elapsed.as_secs() / 60,
                    elapsed.as_secs() % 60
                ));
            });
        match pause_state {
            State::GameInProgress => (),
            State::GamePaused => {
                const MENU_SIZE: egui::Vec2 = egui::vec2(200.0, 150.0);
                egui::Window::new(match game_state {
                    game::State::InProgress => "Game paused",
                    game::State::Won => "You made it through!",
                    game::State::Lost { .. } => "Oops...",
                })
                .collapsible(false)
                .resizable(false)
                .fixed_size(MENU_SIZE)
                .fixed_pos(egui::pos2(
                    (self.width_points - MENU_SIZE.x) / 2.0,
                    (self.height_points - MENU_SIZE.y) / 2.0,
                ))
                .show(&self.ctx, |ui| {
                    ui.vertical_centered_justified(|ui| {
                        ui.spacing_mut().button_padding.y = 10.0;
                        match game_state {
                            game::State::InProgress => {
                                if ui.button("Resume").clicked() {
                                    println!("Resuming game");
                                    actions.push(UiAction::ResumeGame);
                                }
                            }
                            game::State::Won => {
                                ui.add_space(10.0);
                                ui.label(format!(
                                    "Your time: {:02}:{:02}.{:03}",
                                    elapsed.as_secs() / 60,
                                    elapsed.as_secs() % 60,
                                    elapsed.as_millis() % 1000
                                ));
                                ui.add_space(10.0);
                            }
                            game::State::Lost { .. } => (),
                        }
                        if ui
                            .button(match game_state {
                                game::State::InProgress => "Restart",
                                game::State::Won => "Play again",
                                game::State::Lost { .. } => "Try again",
                            })
                            .clicked()
                        {
                            println!("Restarting level");
                            actions.push(UiAction::RestartLevel);
                        }
                        if ui.button("Quit").clicked() {
                            println!("Quitting");
                            actions.push(UiAction::Quit);
                        }
                    });
                });
            }
        }
        let (_output, shapes) = self.ctx.end_frame();
        let egui_texture = self.ctx.texture();
        let texture = match &self.texture {
            Some(t) if t.version == egui_texture.version => t,
            _ => {
                println!(
                    "Creating egui texture {} x {}",
                    egui_texture.width, egui_texture.height
                );
                self.texture = Some(EguiTexture {
                    version: egui_texture.version,
                    texture: Rc::new(
                        gfx.create_texture(
                            "egui texture",
                            egui_texture.width as u32,
                            egui_texture.height as u32,
                            &egui_texture
                                .pixels
                                .iter()
                                .flat_map(|&x| std::iter::repeat(x).take(4))
                                .collect::<Vec<u8>>(),
                        ),
                    ),
                });
                self.texture.as_ref().unwrap()
            }
        };
        let objects: Vec<graphics::Object2d> = self
            .ctx
            .tessellate(shapes)
            .iter()
            .map(|clipped_mesh| {
                // TODO: it could be more efficient to reuse the buffers instead of creating new ones each update
                let mut obj = gfx.create_object_2d(
                    &Rc::new(
                        gfx.create_shape_2d(
                            "egui mesh",
                            &clipped_mesh
                                .1
                                .vertices
                                .iter()
                                .map(|v| graphics::Vertex2d {
                                    position: [v.pos.x, v.pos.y],
                                    tex_coords: [v.uv.x, v.uv.y],
                                    color: v.color.to_array(),
                                })
                                .collect::<Vec<graphics::Vertex2d>>(),
                            &clipped_mesh.1.indices,
                        ),
                    ),
                    &texture.texture,
                );
                // X and Y have opposite signs here as egui coordinate system has Y direction opposite our graphics code
                obj.set_scaling(2.0 / self.width_points, -2.0 / self.height_points);
                obj.set_position(-1.0, 1.0);
                obj
            })
            .collect();
        UiOutput { actions, objects }
    }

    fn cursor_moved(&mut self, x: f32, y: f32) {
        self.events.push(egui::Event::PointerMoved(egui::pos2(
            x / self.scale,
            y / self.scale,
        )));
    }

    fn click(&mut self, x: f32, y: f32, pressed: bool) {
        self.events.push(egui::Event::PointerButton {
            pos: egui::pos2(x / self.scale, y / self.scale),
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        });
    }
}

struct EguiTexture {
    texture: Rc<graphics::Texture>,
    version: u64,
}

enum UiAction {
    ResumeGame,
    RestartLevel,
    Quit,
}

struct UiOutput {
    actions: Vec<UiAction>,
    objects: Vec<graphics::Object2d>,
}
