use instant::Instant;
use nalgebra_glm as glm;
use std::time::Duration;
use winit::dpi::PhysicalPosition;
use winit::event::DeviceEvent;
use winit::event::ElementState;
use winit::event::Event;
use winit::event::Touch;
use winit::event::VirtualKeyCode;
use winit::event::WindowEvent;
use winit::event_loop::ControlFlow;
use winit::window::Window;

use crate::game;
use crate::graphics;
use crate::ui;

type WinitEvent<'a> = Event<'a, ()>;

pub struct GameLoop {
    window: Window,
    game: game::Game,
    gfx: graphics::Instance,
    ui: ui::Instance,
    scene: graphics::Scene,
    board_node_id: graphics::NodeId,
    ball_node_id: graphics::NodeId,
    static_camera: bool,
    state: State,
    double_tap_start_t: Option<Instant>,
    last_touch_pos: Option<PhysicalPosition<f64>>,
    timer: Stopwatch,
    stats: Option<Stats>,
}

impl GameLoop {
    pub fn new(
        window: Window,
        game: game::Game,
        gfx: graphics::Instance,
        ui: ui::Instance,
        scene: graphics::Scene,
        board_node_id: graphics::NodeId,
        ball_node_id: graphics::NodeId,
        static_camera: bool,
        print_stats: bool,
    ) -> GameLoop {
        GameLoop {
            window,
            game,
            gfx,
            ui,
            scene,
            board_node_id,
            ball_node_id,
            static_camera,
            state: State::GameInProgress,
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
        }
    }

    pub fn handle_event(&mut self, event: &WinitEvent) -> ControlFlow {
        match event {
            e if is_window_close(e) => return ControlFlow::Exit,
            e if is_esc_key_press(e) => return ControlFlow::Exit,
            Event::Suspended => self.suspended(),
            Event::Resumed => self.resumed(),
            e if is_mouse_click(e) => self.mouse_click(),
            Event::DeviceEvent {
                event: DeviceEvent::MouseMotion { delta },
                ..
            } => self.mouse_moved(delta),
            Event::WindowEvent {
                event: WindowEvent::Touch(touch),
                ..
            } => self.touch(touch),
            Event::MainEventsCleared => self.do_frame(),
            _ => (),
        }

        return match self.state {
            State::GameInProgress => ControlFlow::Poll,
            State::GamePaused => ControlFlow::Wait,
        };
    }

    fn do_frame(&mut self) {
        match self.state {
            State::GameInProgress => {
                let now = Instant::now();
                let ball_pos_delta = self.update_game(now);
                self.update_scene(now, ball_pos_delta);
                self.gfx.render_scene(
                    &self.scene,
                    &self.ui.update(&self.gfx, self.timer.elapsed()),
                );
                self.update_frame_stats(now);
            }
            State::GamePaused => (),
        }
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
        self.gfx.render_scene(
            &self.scene,
            &self.ui.update(&self.gfx, self.timer.elapsed()),
        );
    }

    fn mouse_click(&mut self) {
        self.toggle_pause();
    }

    fn mouse_moved(&mut self, delta: &(f64, f64)) {
        match self.state {
            State::GameInProgress => {
                const ROTATE_COEFF: f32 = 0.0002;
                self.game.rotate_x(ROTATE_COEFF * delta.0 as f32);
                self.game.rotate_y(ROTATE_COEFF * delta.1 as f32);
            }
            State::GamePaused => (),
        }
    }

    fn touch(&mut self, touch: &Touch) {
        match touch.phase {
            winit::event::TouchPhase::Started => self.touch_started(touch.location),
            winit::event::TouchPhase::Moved => self.swipe(touch.location),
            _ => (),
        }
    }

    fn update_game(&mut self, now: Instant) -> glm::Vec3 {
        let p0 = self.game.ball_pos;
        self.game.update(now);
        match self.game.state {
            game::State::InProgress => (),
            _ => self.timer.stop(),
        }
        return glm::vec3(
            self.game.ball_pos.x - p0.x,
            0.0,
            self.game.ball_pos.y - p0.y,
        );
    }

    fn update_scene(&mut self, now: Instant, ball_pos_delta: glm::Vec3) {
        self.update_board();
        match self.game.state {
            game::State::InProgress => {
                let ball_pos = self.ball_pos_in_scene();
                self.update_ball(ball_pos, ball_pos_delta);
                self.update_camera(ball_pos);
            }
            game::State::Lost { hole, t_lost } => self.update_ball_game_lost(now, t_lost, hole),
            game::State::Won => (),
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

    fn update_ball_game_lost(&mut self, now: Instant, t_lost: Instant, hole_pos: game::Point) {
        match animate_ball_falling_in_hole(
            now.duration_since(t_lost).as_secs_f32(),
            self.game.ball_pos,
            hole_pos,
        ) {
            Some((x, y, z)) => self.scene.get_node(self.ball_node_id).set_position(
                x - self.game.level.size.w / 2.0,
                z,
                y - self.game.level.size.h / 2.0,
            ),
            None => self
                .scene
                .get_node(self.ball_node_id)
                .set_scaling(0.0, 0.0, 0.0),
        }
    }

    fn touch_started(&mut self, pos: PhysicalPosition<f64>) {
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

    fn swipe(&mut self, pos: PhysicalPosition<f64>) {
        match self.state {
            State::GameInProgress => {
                if let Some(p0) = self.last_touch_pos {
                    const ROTATE_COEFF: f32 = 0.0004;
                    self.game.rotate_x(ROTATE_COEFF * (pos.x - p0.x) as f32);
                    self.game.rotate_y(ROTATE_COEFF * (pos.y - p0.y) as f32);
                }
            }
            State::GamePaused => (),
        }
        self.last_touch_pos = Some(pos);
    }

    fn double_tap(&mut self) {
        self.toggle_pause();
    }

    fn toggle_pause(&mut self) {
        match self.state {
            State::GameInProgress => self.pause_game(),
            State::GamePaused => self.resume_game(),
        }
    }

    fn pause_game(&mut self) {
        self.state = State::GamePaused;
        self.game.reset_time();
        self.timer.stop();
    }

    fn resume_game(&mut self) {
        self.state = State::GameInProgress;
        self.timer.start();
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

fn is_window_close(event: &WinitEvent) -> bool {
    match event {
        Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } => true,
        _ => false,
    }
}

fn is_mouse_click(event: &WinitEvent) -> bool {
    match event {
        Event::WindowEvent {
            event:
                WindowEvent::MouseInput {
                    state: ElementState::Pressed,
                    ..
                },
            ..
        } => true,
        _ => false,
    }
}

fn is_esc_key_press(event: &WinitEvent) -> bool {
    match event {
        Event::WindowEvent {
            event:
                WindowEvent::KeyboardInput {
                    input:
                        winit::event::KeyboardInput {
                            virtual_keycode: Some(VirtualKeyCode::Escape),
                            state: winit::event::ElementState::Pressed,
                            ..
                        },
                    ..
                },
            ..
        } => true,
        _ => false,
    }
}
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
