#[macro_use]
extern crate glium;
use glium::glutin;
use glutin::event::{DeviceEvent, Event, StartCause, WindowEvent};
use std::env;
use std::rc::Rc;
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
    shape: &Rc<graphics::Shape>,
    texture: &Rc<graphics::Texture>) -> graphics::Node
{
    let mut node = graphics::Node::object(shape, texture);
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
            Side::Left | Side::Right  => 0.0,
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
        let u_max = (u_mid + hole_r) as usize;
        let u_min = (u_mid - hole_r) as usize;
        let v_min = (v_mid - hole_r) as usize;
        let v_max = (v_mid + hole_r) as usize;
        for u in u_min .. u_max + 1 {
            for v in v_min .. v_max + 1 {
                if (u_mid - u as f32).powi(2) + (v_mid - v as f32).powi(2) < hole_r.powi(2) {
                    *img.pixel(u, v).a() = 0;
                }
            }
        }
    }
}

fn main() {
    let fullscreen = env::args().any(|arg| arg == "-f");
    let static_camera = env::args().any(|arg| arg == "-s");

    let event_loop = glutin::event_loop::EventLoop::new();
    let wb = glutin::window::WindowBuilder::new();
    let cb = glutin::ContextBuilder::new().with_depth_buffer(24).with_multisampling(2);
    let display = glium::Display::new(wb, cb, &event_loop).unwrap();

    let mut w = display.gl_window().window().inner_size().width as f32;
    let mut h = display.gl_window().window().inner_size().height as f32;

    if fullscreen {
        let monitor = display.gl_window().window().available_monitors().next().unwrap();
        w = monitor.size().width as f32;
        h = monitor.size().height as f32;
        display.gl_window().window().set_fullscreen(Some(glutin::window::Fullscreen::Borderless(monitor)));
    }

    println!("Window size {} x {}", w, h);

    display.gl_window().window().set_cursor_position(
        glutin::dpi::PhysicalPosition::new(w / 2.0, h / 2.0)).unwrap();
    display.gl_window().window().set_cursor_visible(false);
    display.gl_window().window().set_cursor_grab(true).unwrap();

    // Create a level and set up the scene based on it
    let level1 = game::Level::from_json("level1.json");
    let level1_half_w = level1.size.w / 2.0;
    let level1_half_h = level1.size.h / 2.0;

    // Shapes
    let quad = Rc::new(graphics::Shape::from_ply(&display, "quad.ply"));
    let cube = Rc::new(graphics::Shape::from_ply(&display, "cube.ply"));
    let sphere = Rc::new(graphics::Shape::from_ply(&display, "sphere.ply"));

    // Textures
    let outer_wall_tex = Rc::new(graphics::Texture::from_image(&display, graphics::Image::solid_color(191, 140, 77)));
    let inner_wall_tex = Rc::new(graphics::Texture::from_image(&display, graphics::Image::solid_color(26, 26, 26)));
    let ball_tex = Rc::new(graphics::Texture::from_image(&display, graphics::Image::solid_color(153, 153, 153)));
    let mut board_tex_image = graphics::Image::solid_color_sized(
        191, 140, 77,
        level1.size.w as u32,
        level1.size.h as u32
    );
    punch_holes(&mut board_tex_image, &level1);
    let board_tex = Rc::new(graphics::Texture::from_image(&display, board_tex_image));
    let board_markings_tex = Rc::new(graphics::Texture::from_image(&display, graphics::Image::from_file("level1_markings.png")));

    // The scene
    let mut scene = graphics::Scene::new(&display, w / h);

    // Board outer walls
    let outer_wall_area = game::Size { w: level1.size.w + 3.0 * BOARD_WALL_W, h: level1.size.h + 3.0 * BOARD_WALL_W };
    scene.add_node(board_wall(Side::Left, &outer_wall_area, &cube, &outer_wall_tex), None);
    scene.add_node(board_wall(Side::Right, &outer_wall_area, &cube, &outer_wall_tex), None);
    scene.add_node(board_wall(Side::Top, &outer_wall_area, &cube, &outer_wall_tex), None);
    scene.add_node(board_wall(Side::Bottom, &outer_wall_area, &cube, &outer_wall_tex), None);

    // Parent node for board moving parts
    let board = graphics::Node::transformation();
    let board_id = scene.add_node(board, None);

    // Ball (has to be added to the scene before the board surface to draw ball falling in to hole correctly)
    let mut ball = graphics::Node::object(&sphere, &ball_tex);
    ball.set_scaling(game::BALL_R, game::BALL_R, game::BALL_R);
    ball.set_position(level1.start.x - level1_half_w, game::BALL_R, level1.start.y - level1_half_h);
    let ball_id = scene.add_node(ball, Some(board_id));

    // Board surface
    let mut board_surface = graphics::Node::object(&quad, &board_tex);
    board_surface.set_scaling(level1.size.w, 1.0, level1.size.h);
    scene.add_node(board_surface, Some(board_id));

    // Board markings
    let mut board_markings = graphics::Node::object(&quad, &board_markings_tex);
    board_markings.set_scaling(level1.size.w, 1.0, level1.size.h);
    // lift the marking very slightly above the board surface so that there's no z-fighting and the markings are visible
    board_markings.set_position(0.0, game::BALL_R / 100.0, 0.0);
    scene.add_node(board_markings, Some(board_id));

    // Board edge walls
    scene.add_node(board_wall(Side::Left, &level1.size, &cube, &outer_wall_tex), Some(board_id));
    scene.add_node(board_wall(Side::Right, &level1.size, &cube, &outer_wall_tex), Some(board_id));
    scene.add_node(board_wall(Side::Top, &level1.size, &cube, &outer_wall_tex), Some(board_id));
    scene.add_node(board_wall(Side::Bottom, &level1.size, &cube, &outer_wall_tex), Some(board_id));

    // Walls
    for wall in level1.walls.iter() {
        let mut obj = graphics::Node::object(&cube, &inner_wall_tex);
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
        0.0, 1.2 * level1.size.w.max(level1.size.h), 0.1,
        0.0, 0.0, 0.0
    );

    // Create a new game from the level and enter the main event loop
    let mut game = game::Game::new(level1);

    event_loop.run(move |event, _, control_flow| {
        // Wake up after a deadline if no other events are received
        *control_flow = glutin::event_loop::ControlFlow::WaitUntil(Instant::now() + Duration::from_nanos(16_666_667));

        match event {
            Event::DeviceEvent { event, .. } => match event {
                DeviceEvent::MouseMotion { delta } => {
                    const ROTATE_COEFF: f32 = 0.0002;
                    game.rotate_x(ROTATE_COEFF * delta.0 as f32);
                    game.rotate_y(ROTATE_COEFF * delta.1 as f32);
                    scene.get_node(board_id).set_rotation(game.angle_y, 0.0, -game.angle_x);
                    // -> proceed to update game state and draw
                },
                _ => return,
            },
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    *control_flow = glutin::event_loop::ControlFlow::Exit;
                    return;
                },
                WindowEvent::KeyboardInput {
                    input: glutin::event::KeyboardInput {
                        virtual_keycode: Some(glutin::event::VirtualKeyCode::Escape),
                        state: glutin::event::ElementState::Pressed,
                        ..
                    },
                    ..
                } => {
                    *control_flow = glutin::event_loop::ControlFlow::Exit;
                    return;
                },
                _ => return,
            },
            Event::NewEvents(cause) => match cause {
                // woke up after deadline -> proceed to update game state and draw
                StartCause::ResumeTimeReached { .. } => (), 
                _ => return,
            },
            _ => return,
        }

        let now = Instant::now();
        game.update(now);

        match game.state {
            game::State::InProgress => {
                scene.get_node(ball_id).set_position(
                    game.ball_pos.x - level1_half_w,
                    game::BALL_R,
                    game.ball_pos.y - level1_half_h,
                );
                if !static_camera {
                    scene.look_at(
                        game.ball_pos.x - level1_half_w, 40.0 * game::BALL_R, game.ball_pos.y - level1_half_h + 10.0 * game::BALL_R,
                        game.ball_pos.x - level1_half_w, 0.0, game.ball_pos.y - level1_half_h,
                    );
                }
            },
            game::State::Lost { hole, t_lost } => {
                match animate_ball_falling_in_hole(now.duration_since(t_lost).as_secs_f32(), game.ball_pos, hole) {
                    Some((x, y, z)) => scene.get_node(ball_id).set_position(x - level1_half_w, z, y - level1_half_h),
                    None => scene.get_node(ball_id).set_scaling(0.0, 0.0, 0.0),
                }
            },
            _ => (),
        }

        let mut target = display.draw();
        scene.draw(&mut target);
        target.finish().unwrap();
    });
}

use nalgebra_glm as glm;

// Calculates the ball position (x, y, z) when the game has been lost and the ball is falling in to hole.
// x and y are in game coordinates, z is the vertical distance from the game's board surface.
// The animation has finite duration and `None` is returned when the animation has finished.
// `t` is the duration since (in s), and `last_ball_pos` is the ball position when the game was lost.
// `hole_pos` is the center of the hole where the ball is falling.
fn animate_ball_falling_in_hole(t: f32, last_ball_pos: game::Point, hole_pos: game::Point) -> Option<(f32, f32, f32)> {
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
        },
        t if t < TOTAL_DURATION => Some((
            free_fall_point.x,
            free_fall_point.y,
            -(t - ROLL_OVER_DURATION) / FREE_FALL_DURATION * FREE_FALL_DEPTH,
        )),
        _ => None,
    }
}
