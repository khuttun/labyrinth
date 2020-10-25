use std::rc::Rc;
use std::str::FromStr;
use std::time::{Duration, Instant};
mod game;
mod graphics;

// TODO: Move utility functions out from main

const BOARD_WALL_W: f32 = game::BALL_R; // width of board edge walls
const WALL_H: f32 = game::BALL_R; // height of all walls

enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

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
pub fn punch_holes(img: &mut graphics::Image, level: &game::Level) {
    let scale = img.w as f32 / level.size.w;
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
                    *img.pixel(u, v).a() = 0;
                }
            }
        }
    }
}

fn main() {
    let args = clap::App::new("labyrinth")
        .args_from_usage(
            "-f                    'Sets fullscreen mode'
            -s                    'Sets static camera'
            -m, --mipmap=[LEVELS] 'Sets the number of texture mipmap levels to use'",
        )
        .get_matches();

    let fullscreen = args.is_present("f");
    let static_camera = args.is_present("s");

    let mut gfx_cfg = graphics::Config::new();
    if let Some(val) = args.value_of("mipmap") {
        gfx_cfg.mipmap_levels = u32::from_str(val).expect("Invalid mipmap levels option");
    }

    let event_loop = winit::event_loop::EventLoop::new();
    let window = winit::window::Window::new(&event_loop).unwrap();

    let mut w = window.inner_size().width;
    let mut h = window.inner_size().height;

    if fullscreen {
        let monitor = window.available_monitors().next();
        w = monitor.as_ref().unwrap().size().width;
        h = monitor.as_ref().unwrap().size().height;
        window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(monitor)));
    }

    println!("Window size {} x {}", w, h);

    window
        .set_cursor_position(winit::dpi::PhysicalPosition::new(w / 2, h / 2))
        .unwrap();
    window.set_cursor_visible(false);
    window.set_cursor_grab(true).unwrap();

    // Create a level and set up the scene based on it
    let level1 = game::Level::from_json("level1.json");
    let level1_half_w = level1.size.w / 2.0;
    let level1_half_h = level1.size.h / 2.0;

    let mut gfx = futures::executor::block_on(graphics::Instance::new(gfx_cfg, &window, w, h));

    // Shapes
    let quad = Rc::new(gfx.create_shape("quad.ply"));
    let cube = Rc::new(gfx.create_shape("cube.ply"));
    let sphere = Rc::new(gfx.create_shape("sphere.ply"));

    // Textures
    println!("Loading wall texture...");
    let wall_tex = Rc::new(gfx.create_texture(&graphics::Image::from_file("wall.jpg")));
    println!("Loading ball texture...");
    let ball_tex = Rc::new(gfx.create_texture(&graphics::Image::from_file("ball.jpg")));
    println!("Loading board texture image...");
    let mut board_tex_image = graphics::Image::from_file("board.jpg");
    println!("Adding holes to the board texture image...");
    punch_holes(&mut board_tex_image, &level1);
    println!("Creating board texture...");
    let board_tex = Rc::new(gfx.create_texture(&board_tex_image));
    println!("Creating board markings texture...");
    let board_markings_tex =
        Rc::new(gfx.create_texture(&graphics::Image::from_file("level1_markings.png")));
    println!("Textures done");

    // The scene
    let mut scene = gfx.create_scene();

    // Board outer walls
    let outer_wall_area = game::Size {
        w: level1.size.w + 3.0 * BOARD_WALL_W,
        h: level1.size.h + 3.0 * BOARD_WALL_W,
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
        level1.start.x - level1_half_w,
        game::BALL_R,
        level1.start.y - level1_half_h,
    );
    let ball_id = scene.add_node(ball, Some(board_id));

    // Board surface
    let mut board_surface = gfx.create_object(&quad, &board_tex);
    board_surface.set_scaling(level1.size.w, 1.0, level1.size.h);
    scene.add_node(board_surface, Some(board_id));

    // Board markings
    let mut board_markings = gfx.create_object(&quad, &board_markings_tex);
    board_markings.set_scaling(level1.size.w, 1.0, level1.size.h);
    // lift the marking very slightly above the board surface so that there's no z-fighting and the markings are visible
    board_markings.set_position(0.0, game::BALL_R / 100.0, 0.0);
    scene.add_node(board_markings, Some(board_id));

    // Board edge walls
    scene.add_node(
        board_wall(Side::Left, &level1.size, &gfx, &cube, &wall_tex),
        Some(board_id),
    );
    scene.add_node(
        board_wall(Side::Right, &level1.size, &gfx, &cube, &wall_tex),
        Some(board_id),
    );
    scene.add_node(
        board_wall(Side::Top, &level1.size, &gfx, &cube, &wall_tex),
        Some(board_id),
    );
    scene.add_node(
        board_wall(Side::Bottom, &level1.size, &gfx, &cube, &wall_tex),
        Some(board_id),
    );

    // Walls
    for wall in level1.walls.iter() {
        let mut obj = gfx.create_object(&cube, &wall_tex);
        obj.set_scaling(wall.size.w, WALL_H, wall.size.h);
        obj.set_position(
            wall.pos.x - level1_half_w + wall.size.w / 2.0,
            WALL_H / 2.0,
            wall.pos.y - level1_half_h + wall.size.h / 2.0,
        );
        scene.add_node(obj, Some(board_id));
    }

    // Set light directly above the board
    scene.set_light_position(0.0, level1_half_w.max(level1_half_h), 0.0);

    // Set initial camera position
    scene.look_at(
        0.0,
        1.2 * level1.size.w.max(level1.size.h),
        0.1,
        0.0,
        0.0,
        0.0,
    );

    // Create a new game from the level and enter the main event loop
    let mut game = game::Game::new(level1);

    event_loop.run(move |event, _, control_flow| {
        // Wake up after a deadline if no other events are received
        *control_flow = winit::event_loop::ControlFlow::WaitUntil(
            Instant::now() + Duration::from_nanos(16_666_667),
        );

        match event {
            winit::event::Event::DeviceEvent { event, .. } => match event {
                winit::event::DeviceEvent::MouseMotion { delta } => {
                    const ROTATE_COEFF: f32 = 0.0002;
                    game.rotate_x(ROTATE_COEFF * delta.0 as f32);
                    game.rotate_y(ROTATE_COEFF * delta.1 as f32);
                    scene
                        .get_node(board_id)
                        .set_rotation(game.angle_y, 0.0, -game.angle_x);
                    // -> proceed to update game state and draw
                }
                _ => return,
            },
            winit::event::Event::WindowEvent { event, .. } => match event {
                winit::event::WindowEvent::CloseRequested => {
                    *control_flow = winit::event_loop::ControlFlow::Exit;
                    return;
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
                    return;
                }
                _ => return,
            },
            winit::event::Event::NewEvents(cause) => match cause {
                // woke up after deadline -> proceed to update game state and draw
                winit::event::StartCause::ResumeTimeReached { .. } => (),
                _ => return,
            },
            _ => return,
        }

        let p0 = game.ball_pos;

        let now = Instant::now();
        game.update(now);

        let ball_pos_delta = glm::vec3(game.ball_pos.x - p0.x, 0.0, game.ball_pos.y - p0.y);

        match game.state {
            game::State::InProgress => {
                // Ball position
                scene.get_node(ball_id).set_position(
                    game.ball_pos.x - level1_half_w,
                    game::BALL_R,
                    game.ball_pos.y - level1_half_h,
                );

                // Ball rotation
                let axis_ws = glm::normalize(&glm::rotate_vec3(
                    &ball_pos_delta,
                    std::f32::consts::PI / 2.0,
                    &glm::vec3(0.0, 1.0, 0.0),
                ));
                let axis = glm::normalize(
                    &(glm::inverse(&scene.get_node(ball_id).model_matrix)
                        * glm::vec3_to_vec4(&axis_ws))
                    .xyz(),
                );
                scene.get_node(ball_id).rotate(
                    glm::length(&ball_pos_delta) / game::BALL_R,
                    axis.x,
                    axis.y,
                    axis.z,
                );

                // Camera movement
                if !static_camera {
                    scene.look_at(
                        game.ball_pos.x - level1_half_w,
                        40.0 * game::BALL_R,
                        game.ball_pos.y - level1_half_h + 10.0 * game::BALL_R,
                        game.ball_pos.x - level1_half_w,
                        0.0,
                        game.ball_pos.y - level1_half_h,
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
                        x - level1_half_w,
                        z,
                        y - level1_half_h,
                    ),
                    None => scene.get_node(ball_id).set_scaling(0.0, 0.0, 0.0),
                }
            }
            _ => (),
        }

        gfx.render_scene(&scene);
    });
}

use nalgebra_glm as glm;

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
