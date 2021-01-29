use instant::Instant;
use mobile_entry_point::mobile_entry_point;
use nalgebra_glm as glm;
use std::rc::Rc;
use std::str::FromStr;
use std::time::Duration;

mod game;
mod graphics;

#[mobile_entry_point]
pub fn run() {
    #[cfg(target_os = "android")]
    android_logger::init_once(
        android_logger::Config::default()
            .with_min_level(log::Level::Info)
            .with_tag("labyrinth"),
    );

    let args = clap::App::new("labyrinth")
        .args_from_usage(
            "-f                    'Sets fullscreen mode'
            -s                    'Sets static camera'
            -t                    'Enables statistics output'
            -m, --mipmap=[LEVELS] 'Sets the number of texture mipmap levels to use'
            -n, --no-vsync        'Disables VSync for unlimited FPS'",
        )
        .get_matches();

    let fullscreen = args.is_present("f");
    let static_camera = args.is_present("s");
    let stats = args.is_present("t");

    let mut gfx_cfg = graphics::Config::new();
    if let Some(val) = args.value_of("mipmap") {
        gfx_cfg.mipmap_levels = u32::from_str(val).expect("Invalid mipmap levels option");
    }

    if args.is_present("no-vsync") {
        gfx_cfg.vsync = false;
    }

    let event_loop = winit::event_loop::EventLoop::new();
    let window = winit::window::Window::new(&event_loop).expect("Failed to create window");

    // On Android, the window can be used only after the activity has properly started.
    // TODO: Could this be handled by waiting for winit::event::Event::Resumed?
    // https://github.com/rust-windowing/winit/issues/1588
    #[cfg(target_os = "android")]
    std::thread::sleep(std::time::Duration::from_secs(2));

    let mut w = window.inner_size().width;
    let mut h = window.inner_size().height;

    if fullscreen {
        let monitor = window.available_monitors().next();
        w = monitor.as_ref().unwrap().size().width;
        h = monitor.as_ref().unwrap().size().height;
        window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(monitor)));
    }

    println!("Window size {} x {}", w, h);

    // On wasm, append the canvas to the document body
    #[cfg(target_arch = "wasm32")]
    {
        use winit::platform::web::WindowExtWebSys;
        console_log::init().expect("Failed to initialize logger");
        std::panic::set_hook(Box::new(console_error_panic_hook::hook));
        web_sys::window()
            .and_then(|win| win.document())
            .and_then(|doc| doc.body())
            .and_then(|body| {
                body.append_child(&web_sys::Element::from(window.canvas()))
                    .ok()
            })
            .expect("Failed to append canvas to document body");
    }

    window.set_cursor_visible(false);

    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    {
        window
            .set_cursor_position(winit::dpi::PhysicalPosition::new(w / 2, h / 2))
            .expect("Failed center cursor");
        window.set_cursor_grab(true).expect("Failed to grab cursor");
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut gfx = futures::executor::block_on(graphics::Instance::new(gfx_cfg, w, h));
        gfx.set_window(Some(&window));
        play(gfx, event_loop, static_camera, stats);
    }
    #[cfg(target_arch = "wasm32")]
    {
        wasm_bindgen_futures::spawn_local(async move {
            let mut gfx = graphics::Instance::new(gfx_cfg, w, h).await;
            gfx.set_window(Some(&window));
            play(gfx, event_loop, static_camera, stats);
        });
    }
}

fn play(
    gfx: graphics::Instance,
    event_loop: winit::event_loop::EventLoop<()>,
    static_camera: bool,
    stats: bool,
) {
    let level = game::Level::from_json(include_str!("level1.json"));
    let level_half_w = level.size.w / 2.0;
    let level_half_h = level.size.h / 2.0;

    // Shapes
    let quad = Rc::new(gfx.create_shape("quad", include_str!("quad.ply")));
    let cube = Rc::new(gfx.create_shape("cube", include_str!("cube.ply")));
    let sphere = Rc::new(gfx.create_shape("sphere", include_str!("sphere.ply")));

    // Textures
    println!("Loading wall texture...");
    let wall_img = create_image(include_bytes!("wall.jpg"), image::ImageFormat::Jpeg);
    let wall_tex =
        Rc::new(gfx.create_texture("wall", wall_img.width(), wall_img.height(), &wall_img));
    println!("Loading ball texture...");
    let ball_img = create_image(include_bytes!("ball.jpg"), image::ImageFormat::Jpeg);
    let ball_tex =
        Rc::new(gfx.create_texture("ball", ball_img.width(), ball_img.height(), &ball_img));
    println!("Loading board texture...");
    let mut board_img = create_image(include_bytes!("board.jpg"), image::ImageFormat::Jpeg);
    punch_holes(&mut board_img, &level);
    let board_tex =
        Rc::new(gfx.create_texture("board", board_img.width(), board_img.height(), &board_img));
    println!("Loading board markings texture...");
    let board_markings_img = create_image(
        include_bytes!("level1_markings.png"),
        image::ImageFormat::Png,
    );
    let board_markings_tex = Rc::new(gfx.create_texture(
        "markings",
        board_markings_img.width(),
        board_markings_img.height(),
        &board_markings_img,
    ));
    println!("Textures done");

    // The scene
    let mut scene = gfx.create_scene();

    // Board outer walls
    let outer_wall_area = game::Size {
        w: level.size.w + 3.0 * BOARD_WALL_W,
        h: level.size.h + 3.0 * BOARD_WALL_W,
    };
    scene.add_node(
        board_wall(Side::Left, &outer_wall_area, &gfx, &cube, &wall_tex),
        None,
    );
    scene.add_node(
        board_wall(Side::Right, &outer_wall_area, &gfx, &cube, &wall_tex),
        None,
    );
    scene.add_node(
        board_wall(Side::Top, &outer_wall_area, &gfx, &cube, &wall_tex),
        None,
    );
    scene.add_node(
        board_wall(Side::Bottom, &outer_wall_area, &gfx, &cube, &wall_tex),
        None,
    );

    // Parent node for board moving parts
    let board = gfx.create_transformation();
    let board_id = scene.add_node(board, None);

    // Ball (has to be added to the scene before the board surface to draw ball falling in to hole correctly)
    let mut ball = gfx.create_object(&sphere, &ball_tex);
    ball.set_scaling(game::BALL_R, game::BALL_R, game::BALL_R);
    ball.set_position(
        level.start.x - level_half_w,
        game::BALL_R,
        level.start.y - level_half_h,
    );
    let ball_id = scene.add_node(ball, Some(board_id));

    // Board surface
    let mut board_surface = gfx.create_object(&quad, &board_tex);
    // extend board surface very slightly below board edge walls so that the background doesn't leak through from the seam
    board_surface.set_scaling(
        level.size.w + BOARD_WALL_W / 100.0,
        1.0,
        level.size.h + BOARD_WALL_W / 100.0,
    );
    scene.add_node(board_surface, Some(board_id));

    // Board markings
    let mut board_markings = gfx.create_object(&quad, &board_markings_tex);
    board_markings.set_scaling(level.size.w, 1.0, level.size.h);
    // lift the marking very slightly above the board surface so that there's no z-fighting and the markings are visible
    board_markings.set_position(0.0, game::BALL_R / 100.0, 0.0);
    scene.add_node(board_markings, Some(board_id));

    // Board edge walls
    scene.add_node(
        board_wall(Side::Left, &level.size, &gfx, &cube, &wall_tex),
        Some(board_id),
    );
    scene.add_node(
        board_wall(Side::Right, &level.size, &gfx, &cube, &wall_tex),
        Some(board_id),
    );
    scene.add_node(
        board_wall(Side::Top, &level.size, &gfx, &cube, &wall_tex),
        Some(board_id),
    );
    scene.add_node(
        board_wall(Side::Bottom, &level.size, &gfx, &cube, &wall_tex),
        Some(board_id),
    );

    // Walls
    for wall in level.walls.iter() {
        let mut obj = gfx.create_object(&cube, &wall_tex);
        obj.set_scaling(wall.size.w, WALL_H, wall.size.h);
        obj.set_position(
            wall.pos.x - level_half_w + wall.size.w / 2.0,
            WALL_H / 2.0,
            wall.pos.y - level_half_h + wall.size.h / 2.0,
        );
        scene.add_node(obj, Some(board_id));
    }

    // Add lights
    gfx.add_light_to(
        &mut scene,
        0.0,
        level.size.w.max(level.size.h),
        -level_half_h,
        0.0,
        0.0,
        0.0,
    );
    gfx.add_light_to(
        &mut scene,
        -level_half_w,
        level.size.w.max(level.size.h),
        level.size.h,
        0.0,
        0.0,
        0.0,
    );
    gfx.add_light_to(
        &mut scene,
        level.size.w,
        0.75 * level.size.w.max(level.size.h),
        level_half_h,
        0.0,
        0.0,
        0.0,
    );

    // Set initial camera position
    scene.look_at(
        0.0,
        1.2 * level.size.w.max(level.size.h),
        0.1,
        0.0,
        0.0,
        0.0,
    );

    // Create a new game from the level
    let mut game = game::Game::new(level);

    // Statistics
    const STATS_INTERVAL: Duration = Duration::from_secs(5);
    let mut stats_t = Instant::now();
    let mut stats_frames = 0;

    // Keep track of the last recorded touch position on touch screens
    let mut last_touch = winit::dpi::PhysicalPosition::new(0.0, 0.0);

    let mut paused = false;

    // Enter the main loop
    event_loop.run(move |event, _, control_flow| {
        // If paused, just wait for an event that would end the pause.
        if paused {
            match event {
                winit::event::Event::WindowEvent {
                    event:
                        winit::event::WindowEvent::MouseInput {
                            state: winit::event::ElementState::Pressed,
                            ..
                        },
                    ..
                } => {
                    paused = false;
                    *control_flow = winit::event_loop::ControlFlow::Poll;
                }
                _ => {
                    *control_flow = winit::event_loop::ControlFlow::Wait;
                }
            }

            return;
        }
        // Use ControlFlow::Poll to wake up event loop immediately again after each iteration.
        // The render code in graphics module will block appropriately so that frames are created at most at
        // the monitor refresh rate.
        *control_flow = winit::event_loop::ControlFlow::Poll;
        match event {
            winit::event::Event::DeviceEvent { event, .. } => match event {
                winit::event::DeviceEvent::MouseMotion { delta } => {
                    const ROTATE_COEFF: f32 = 0.0002;
                    game.rotate_x(ROTATE_COEFF * delta.0 as f32);
                    game.rotate_y(ROTATE_COEFF * delta.1 as f32);
                }
                _ => (),
            },
            winit::event::Event::WindowEvent { event, .. } => match event {
                winit::event::WindowEvent::CloseRequested => {
                    *control_flow = winit::event_loop::ControlFlow::Exit;
                }
                winit::event::WindowEvent::KeyboardInput {
                    input:
                        winit::event::KeyboardInput {
                            virtual_keycode: Some(winit::event::VirtualKeyCode::Escape),
                            state: winit::event::ElementState::Pressed,
                            ..
                        },
                    ..
                } => {
                    *control_flow = winit::event_loop::ControlFlow::Exit;
                }
                winit::event::WindowEvent::MouseInput {
                    state: winit::event::ElementState::Pressed,
                    ..
                } => {
                    paused = true;
                    game.reset_time();
                }
                winit::event::WindowEvent::Touch(touch) => match touch.phase {
                    winit::event::TouchPhase::Started => last_touch = touch.location,
                    winit::event::TouchPhase::Moved => {
                        const ROTATE_COEFF: f32 = 0.0004;
                        game.rotate_x(ROTATE_COEFF * (touch.location.x - last_touch.x) as f32);
                        game.rotate_y(ROTATE_COEFF * (touch.location.y - last_touch.y) as f32);
                        last_touch = touch.location
                    }
                    _ => (),
                },
                _ => (),
            },
            winit::event::Event::MainEventsCleared => {
                // Update game state
                let p0 = game.ball_pos;
                let now = Instant::now();
                game.update(now);
                let ball_pos_delta = glm::vec3(game.ball_pos.x - p0.x, 0.0, game.ball_pos.y - p0.y);

                // Update scene
                scene
                    .get_node(board_id)
                    .set_rotation(game.angle_y, 0.0, -game.angle_x);
                match game.state {
                    game::State::InProgress => {
                        // Ball position
                        scene.get_node(ball_id).set_position(
                            game.ball_pos.x - level_half_w,
                            game::BALL_R,
                            game.ball_pos.y - level_half_h,
                        );

                        // Ball rotation
                        let axis_world_space = glm::normalize(&glm::rotate_vec3(
                            &ball_pos_delta,
                            std::f32::consts::PI / 2.0,
                            &glm::vec3(0.0, 1.0, 0.0),
                        ));
                        scene.get_node(ball_id).rotate_in_world_space(
                            glm::length(&ball_pos_delta) / game::BALL_R,
                            axis_world_space.x,
                            axis_world_space.y,
                            axis_world_space.z,
                        );

                        // Camera movement
                        if !static_camera {
                            scene.look_at(
                                game.ball_pos.x - level_half_w,
                                40.0 * game::BALL_R,
                                game.ball_pos.y - level_half_h + 10.0 * game::BALL_R,
                                game.ball_pos.x - level_half_w,
                                0.0,
                                game.ball_pos.y - level_half_h,
                            );
                        }
                    }
                    game::State::Lost { hole, t_lost } => {
                        match animate_ball_falling_in_hole(
                            now.duration_since(t_lost).as_secs_f32(),
                            game.ball_pos,
                            hole,
                        ) {
                            Some((x, y, z)) => scene.get_node(ball_id).set_position(
                                x - level_half_w,
                                z,
                                y - level_half_h,
                            ),
                            None => scene.get_node(ball_id).set_scaling(0.0, 0.0, 0.0),
                        }
                    }
                    _ => (),
                }

                // Render
                gfx.render_scene(&scene);

                // Statistics
                if stats {
                    stats_frames = stats_frames + 1;
                    let now = Instant::now();
                    let elapsed = now.duration_since(stats_t);
                    if elapsed >= STATS_INTERVAL {
                        let fps = stats_frames as f64 / elapsed.as_secs_f64();
                        println!("FPS {}", fps);
                        stats_t = now;
                        stats_frames = 0;
                    }
                }
            }
            _ => (),
        }
    });
}

const BOARD_WALL_W: f32 = game::BALL_R; // width of board edge walls
const WALL_H: f32 = game::BALL_R; // height of all walls

enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

// Create a board edge wall scene node
fn board_wall(
    side: Side,
    board_size: &game::Size,
    gfx: &graphics::Instance,
    shape: &Rc<graphics::Shape>,
    texture: &Rc<graphics::Texture>,
) -> graphics::Node {
    let mut node = gfx.create_object(shape, texture);
    node.set_scaling(
        match side {
            Side::Left | Side::Right => BOARD_WALL_W,
            Side::Top | Side::Bottom => board_size.w + 2.0 * BOARD_WALL_W,
        },
        WALL_H,
        match side {
            Side::Left | Side::Right => board_size.h,
            Side::Top | Side::Bottom => BOARD_WALL_W,
        },
    );
    node.set_position(
        match side {
            Side::Left => -board_size.w / 2.0 - BOARD_WALL_W / 2.0,
            Side::Right => board_size.w / 2.0 + BOARD_WALL_W / 2.0,
            Side::Top | Side::Bottom => 0.0,
        },
        WALL_H / 2.0,
        match side {
            Side::Left | Side::Right => 0.0,
            Side::Top => board_size.h / 2.0 + BOARD_WALL_W / 2.0,
            Side::Bottom => -board_size.h / 2.0 - BOARD_WALL_W / 2.0,
        },
    );
    return node;
}

// Draw transparent circles to `img` based on the hole locations of `level`.
// The image and the level can be different size but their w/h ratio should be the same.
fn punch_holes(img: &mut image::RgbaImage, level: &game::Level) {
    let scale = img.width() as f32 / level.size.w;
    let hole_r = scale * game::HOLE_R;
    for hole in level.holes.iter() {
        let u_mid = scale * hole.x;
        let v_mid = scale * (level.size.h - hole.y); // board and texture coordinates have opposite y-direction
        let u_max = (u_mid + hole_r) as u32;
        let u_min = (u_mid - hole_r) as u32;
        let v_min = (v_mid - hole_r) as u32;
        let v_max = (v_mid + hole_r) as u32;
        for u in u_min..u_max + 1 {
            for v in v_min..v_max + 1 {
                if (u_mid - u as f32).powi(2) + (v_mid - v as f32).powi(2) < hole_r.powi(2) {
                    img.get_pixel_mut(u, v)[3] = 0;
                }
            }
        }
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

// Create an image suitable for texture use from raw image file bytes
fn create_image(bytes: &[u8], format: image::ImageFormat) -> image::RgbaImage {
    image::load_from_memory_with_format(bytes, format)
        .unwrap()
        .flipv()
        .into_rgba8()
}
