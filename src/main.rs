#[macro_use]
extern crate glium;
use glium::glutin;
use glutin::event::{Event, StartCause, WindowEvent};
use std::f32::consts::PI;
use std::rc::Rc;
use std::time::{Duration, Instant};
mod game;
mod graphics;

const BOARD_TEXTURE_SIZE_FACTOR: f32 = 20.0;
const HOLE_R_IN_TEXTURE: f32 = BOARD_TEXTURE_SIZE_FACTOR * game::HOLE_R;

enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

fn board_wall(side: Side, board_size: &game::Size, display: &glium::Display, shape: &Rc<graphics::Shape>) -> graphics::Node {
    let mut node = graphics::Node::object(display, shape);
    node.set_texture(display, graphics::Texture::solid_color(191, 140, 77));
    node.set_scaling(
        match side {
            Side::Left | Side::Right => game::BALL_R,
            Side::Top | Side::Bottom => board_size.w + 2.0 * game::BALL_R,
        },
        game::WALL_H,
        match side {
            Side::Left | Side::Right => board_size.h,
            Side::Top | Side::Bottom => game::BALL_R,
        },
    );
    node.set_position(
        match side {
            Side::Left => -board_size.w / 2.0 - game::BALL_R / 2.0,
            Side::Right => board_size.w / 2.0 + game::BALL_R / 2.0,
            Side::Top | Side::Bottom => 0.0,
        },
        game::WALL_H / 2.0,
        match side {
            Side::Left | Side::Right  => 0.0,
            Side::Top => board_size.h / 2.0 + game::BALL_R / 2.0,
            Side::Bottom => -board_size.h / 2.0 - game::BALL_R / 2.0,
        },
    );
    return node;
}

pub fn punch_holes(tex: &mut graphics::Texture, holes: &Vec<game::Point>) {
    for hole in holes.iter() {
        let u_mid = BOARD_TEXTURE_SIZE_FACTOR * hole.x;
        let v_mid = tex.h as f32 - BOARD_TEXTURE_SIZE_FACTOR * hole.y; // board and texture coordinates have opposite y-direction
        let u_min = (u_mid - HOLE_R_IN_TEXTURE) as usize;
        let u_max = (u_mid + HOLE_R_IN_TEXTURE) as usize;
        let v_min = (v_mid - HOLE_R_IN_TEXTURE) as usize;
        let v_max = (v_mid + HOLE_R_IN_TEXTURE) as usize;
        for u in u_min .. u_max + 1 {
            for v in v_min .. v_max + 1 {
                if (u_mid - u as f32).powi(2) + (v_mid - v as f32).powi(2) < HOLE_R_IN_TEXTURE.powi(2) {
                    *tex.texel(u, v).a() = 0;
                }
            }
        }
    }
}

fn main() {
    let event_loop = glutin::event_loop::EventLoop::new();
    let wb = glutin::window::WindowBuilder::new();
    let cb = glutin::ContextBuilder::new().with_depth_buffer(24).with_multisampling(2);
    let display = glium::Display::new(wb, cb, &event_loop).unwrap();

    let w = display.gl_window().window().inner_size().width as f32;
    let h = display.gl_window().window().inner_size().height as f32;

    // let monitor = display.gl_window().window().available_monitors().next().unwrap();
    // let w = monitor.size().width as f32;
    // let h = monitor.size().height as f32;
    // display.gl_window().window().set_fullscreen(Some(glutin::window::Fullscreen::Borderless(monitor)));

    println!("Window size {} x {}", w, h);

    display.gl_window().window().set_cursor_position(
        glutin::dpi::PhysicalPosition::new(w / 2.0, h / 2.0)).unwrap();
    display.gl_window().window().set_cursor_visible(false);

    let quad = Rc::new(graphics::Shape::from_ply(&display, "quad.ply"));
    let cube = Rc::new(graphics::Shape::from_ply(&display, "cube.ply"));
    let sphere = Rc::new(graphics::Shape::from_ply(&display, "sphere.ply"));
    
    // Create a level and set up the scene based on it
    let level1 = game::Level::from_json("level1.json");

    let level1_half_w = level1.size.w / 2.0;
    let level1_half_h = level1.size.h / 2.0;

    let mut scene = graphics::Scene::new(&display, w / h);

    // let outer_wall_w = game::BALL_R;
    // let outer_wall_h = 8.0 * game::BALL_R;

    // let mut left_outer_wall = graphics::Node::object(&display, &cube);
    // left_outer_wall.set_texture(&display, graphics::Texture::solid_color(26, 26, 26));
    // left_outer_wall.set_scaling(outer_wall_w, outer_wall_h, level1.size.h);
    // left_outer_wall.set_position(-level1_half_w - outer_wall_w / 2.0, 0.0, 0.0);
    // scene.add_node(left_outer_wall, None);

    // let mut right_outer_wall = graphics::Node::object(&display, &cube);
    // right_outer_wall.set_texture(&display, graphics::Texture::solid_color(26, 26, 26));
    // right_outer_wall.set_scaling(outer_wall_w, outer_wall_h, level1.size.h);
    // right_outer_wall.set_position(level1_half_w + outer_wall_w / 2.0, 0.0, 0.0);
    // scene.add_node(right_outer_wall, None);

    // let mut top_outer_wall = graphics::Node::object(&display, &cube);
    // top_outer_wall.set_texture(&display, graphics::Texture::solid_color(26, 26, 26));
    // top_outer_wall.set_scaling(level1.size.w + 2.0 * outer_wall_w, outer_wall_h, outer_wall_w);
    // top_outer_wall.set_position(0.0, 0.0, -level1_half_h - outer_wall_w / 2.0);
    // scene.add_node(top_outer_wall, None);

    // let mut bottom_outer_wall = graphics::Node::object(&display, &cube);
    // bottom_outer_wall.set_texture(&display, graphics::Texture::solid_color(26, 26, 26));
    // bottom_outer_wall.set_scaling(level1.size.w + 2.0 * outer_wall_w, outer_wall_h, outer_wall_w);
    // bottom_outer_wall.set_position(0.0, 0.0, level1_half_h + outer_wall_w / 2.0);
    // scene.add_node(bottom_outer_wall, None);

    // Parent node for board moving parts
    let board = graphics::Node::transformation();
    let board_id = scene.add_node(board, None);

    // Board surface
    let mut board_surface = graphics::Node::object(&display, &quad);
    let mut board_tex = graphics::Texture::solid_color_sized(
        191, 140, 77,
        (BOARD_TEXTURE_SIZE_FACTOR * level1.size.w) as u32,
        (BOARD_TEXTURE_SIZE_FACTOR * level1.size.h) as u32
    );
    punch_holes(&mut board_tex, &level1.holes);
    board_surface.set_texture(&display, board_tex);
    board_surface.set_scaling(level1.size.w, 1.0, level1.size.h);
    scene.add_node(board_surface, Some(board_id));

    // Board edge walls
    scene.add_node(board_wall(Side::Left, &level1.size, &display, &cube), Some(board_id));
    scene.add_node(board_wall(Side::Right, &level1.size, &display, &cube), Some(board_id));
    scene.add_node(board_wall(Side::Top, &level1.size, &display, &cube), Some(board_id));
    scene.add_node(board_wall(Side::Bottom, &level1.size, &display, &cube), Some(board_id));

    // Ball
    let mut ball = graphics::Node::object(&display, &sphere);
    ball.set_texture(&display, graphics::Texture::solid_color(153, 153, 153));
    ball.set_scaling(game::BALL_R, game::BALL_R, game::BALL_R);
    ball.set_position(level1.start.x - level1_half_w, game::BALL_R, level1.start.y - level1_half_h);
    let ball_id = scene.add_node(ball, Some(board_id));

    // Walls
    for wall in level1.walls.iter() {
        let mut obj = graphics::Node::object(&display, &cube);
        obj.set_texture(&display, graphics::Texture::solid_color(26, 26, 26));
        obj.set_scaling(wall.size.w, game::WALL_H, wall.size.h);
        obj.set_position(
            wall.pos.x - level1_half_w + wall.size.w / 2.0,
            game::WALL_H / 2.0,
            wall.pos.y - level1_half_h + wall.size.h / 2.0,
        );
        scene.add_node(obj, Some(board_id));
    }

    // Set light directly above the board
    scene.set_light_position(0.0, level1_half_w.max(level1_half_h), 0.0);

    // Create a new game from the level and enter the main event loop
    let mut game = game::Game::new(level1);

    event_loop.run(move |event, _, control_flow| {
        // Wake up after a deadline if no other events are received
        *control_flow = glutin::event_loop::ControlFlow::WaitUntil(Instant::now() + Duration::from_nanos(16_666_667));

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    *control_flow = glutin::event_loop::ControlFlow::Exit;
                    return;
                },
                WindowEvent::CursorMoved { position, .. } => {
                    let angle_x = (position.x as f32 - w / 2.0) / (w / 2.0) * (PI / 32.0);
                    let angle_y = (position.y as f32 - h / 2.0) / (h / 2.0) * (PI / 32.0);
                    //println!("Cursor position ({}, {}) -> Board angle ({}°, {}°)", position.x, position.y, angle_x * 180.0 / PI, angle_y * 180.0 / PI);
                    game.set_x_angle(angle_x);
                    game.set_y_angle(angle_y);
                    scene.get_node(board_id).set_rotation(angle_y, 0.0, -angle_x);
                    // -> proceed to update game state and draw
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

        game.update(Instant::now());

        match game.state {
            game::State::InProgress => {
                scene.get_node(ball_id).set_position(game.ball_pos.x - level1_half_w, game::BALL_R, game.ball_pos.y - level1_half_h);
                scene.look_at(
                    game.ball_pos.x - level1_half_w, 40.0 * game::BALL_R, game.ball_pos.y - level1_half_h + 30.0 * game::BALL_R,
                    game.ball_pos.x - level1_half_w, 0.0, game.ball_pos.y - level1_half_h);
            },
            _ => (),
        }

        let mut target = display.draw();
        scene.draw(&mut target);
        target.finish().unwrap();
    });
}
