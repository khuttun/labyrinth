#[macro_use]
extern crate glium;
use glium::glutin;
use glutin::event::{Event, StartCause, WindowEvent};
use std::rc::Rc;
use std::time::{Duration, Instant};
mod game;
mod graphics;

fn main() {
    let event_loop = glutin::event_loop::EventLoop::new();
    let wb = glutin::window::WindowBuilder::new();
    let cb = glutin::ContextBuilder::new().with_depth_buffer(24).with_multisampling(2);
    let display = glium::Display::new(wb, cb, &event_loop).unwrap();

    let w = display.gl_window().window().inner_size().width as f32;
    let h = display.gl_window().window().inner_size().height as f32;

    // let monitor = display.gl_window().window().available_monitors().next().unwrap();
    // let w = monitor.size().width;
    // let h = monitor.size().height;
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

    let mut scene = graphics::Scene::new(&display, w / h);

    let mut board = graphics::Object::new(&quad);
    board.set_color(0.75, 0.55, 0.3);
    board.set_scaling(level1.size.w, 1.0, level1.size.h);
    board.set_position(level1.size.w / 2.0, 0.0, level1.size.h / 2.0);
    scene.add_object(board);

    let mut ball = graphics::Object::new(&sphere);
    ball.set_color(0.6, 0.6, 0.6);
    ball.set_scaling(game::BALL_R, game::BALL_R, game::BALL_R);
    ball.set_position(level1.start.x, game::BALL_R, level1.start.y);
    let ball_id = scene.add_object(ball);

    for wall in level1.walls.iter() {
        let mut obj = graphics::Object::new(&cube);
        obj.set_color(0.1, 0.1, 0.1);
        obj.set_scaling(wall.size.w, game::WALL_H, wall.size.h);
        obj.set_position(wall.pos.x + wall.size.w / 2.0, game::WALL_H / 2.0, wall.pos.y + wall.size.h / 2.0);
        scene.add_object(obj);
    }

    scene.set_light_position(level1.size.w / 2.0, level1.size.w.max(level1.size.h) / 2.0, level1.size.h / 2.0);

    scene.look_at(
        level1.start.x, 30.0 * game::BALL_R, level1.start.y + 30.0 * game::BALL_R,
        level1.start.x, 0.0, level1.start.y);

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
                    let angle_x = (position.x as f32 - w / 2.0) / (w / 2.0) * (std::f32::consts::PI / 8.0);
                    let angle_y = (position.y as f32 - h / 2.0) / (h / 2.0) * (std::f32::consts::PI / 8.0);
                    game.set_x_angle(angle_x);
                    game.set_y_angle(angle_y);
                    println!("Board angle ({}°, {}°)",
                        angle_x * 180.0 / std::f32::consts::PI,
                        angle_y * 180.0 / std::f32::consts::PI);
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
            game::State::InProgress { ball_pos, .. } => {
                scene.get_object(ball_id).set_position(ball_pos.x, game::BALL_R, ball_pos.y);
                scene.look_at(
                    ball_pos.x, 30.0 * game::BALL_R, ball_pos.y + 30.0 * game::BALL_R,
                    ball_pos.x, 0.0, ball_pos.y);
            },
            _ => (),
        }

        let mut target = display.draw();
        scene.draw(&mut target);
        target.finish().unwrap();
    });
}
