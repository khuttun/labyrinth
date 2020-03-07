#[macro_use]
extern crate glium;
use glium::glutin;
use std::rc::Rc;
mod game;
mod graphics;

fn main() {
    let event_loop = glutin::event_loop::EventLoop::new();
    let wb = glutin::window::WindowBuilder::new();
    let cb = glutin::ContextBuilder::new().with_depth_buffer(24).with_multisampling(2);
    let display = glium::Display::new(wb, cb, &event_loop).unwrap();

    let monitor = display.gl_window().window().available_monitors().next().unwrap();
    let w = monitor.size().width;
    let h = monitor.size().height;
    println!("Monitor size {} x {}", w, h);
    display.gl_window().window().set_fullscreen(Some(glutin::window::Fullscreen::Borderless(monitor)));

    display.gl_window().window().set_cursor_position(
        glutin::dpi::PhysicalPosition::new(w / 2, h / 2)).unwrap();
    display.gl_window().window().set_cursor_visible(false);

    let quad = Rc::new(graphics::Shape::from_ply(&display, "quad.ply"));
    let cube = Rc::new(graphics::Shape::from_ply(&display, "cube.ply"));
    let sphere = Rc::new(graphics::Shape::from_ply(&display, "sphere.ply"));
    
    let level1 = game::Level::from_json("level1.json");
    //println!("Level 1: {:#?}", level1);

    let mut scene = graphics::Scene::new(&display, w as f32 / h as f32);

    let mut board = graphics::Object::new(&quad);
    board.set_color(0.75, 0.55, 0.3);
    board.set_scaling(level1.size.w, 1.0, level1.size.h);
    board.set_position(level1.size.w / 2.0, 0.0, level1.size.h / 2.0);
    scene.add_object(board);

    let mut ball = graphics::Object::new(&sphere);
    ball.set_color(0.6, 0.6, 0.6);
    ball.set_scaling(game::BALL_R, game::BALL_R, game::BALL_R);
    ball.set_shininess(100.0);
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

    event_loop.run(move |event, _, control_flow| {
        let next_frame_time = std::time::Instant::now() + std::time::Duration::from_nanos(16_666_667);
        *control_flow = glutin::event_loop::ControlFlow::WaitUntil(next_frame_time);

        match event {
            glutin::event::Event::WindowEvent { event, .. } => match event {
                glutin::event::WindowEvent::CloseRequested => {
                    *control_flow = glutin::event_loop::ControlFlow::Exit;
                    return;
                },
                // glutin::event::WindowEvent::CursorMoved { position, .. } => {
                //     scene.get_object(ball_id).set_position(
                //         5.0f32 * ((position.x as f32) - (w as f32) / 2.0f32) / ((w as f32) / 2.0f32),
                //         0.0f32,
                //         5.0f32 * ((position.y as f32) - (h as f32) / 2.0f32) / ((h as f32) / 2.0f32))
                // },
                glutin::event::WindowEvent::KeyboardInput {
                    input: glutin::event::KeyboardInput {
                        virtual_keycode: Some(glutin::event::VirtualKeyCode::Escape),
                        state: glutin::event::ElementState::Pressed,
                        ..
                    },
                    ..
                } => {
                    *control_flow = glutin::event_loop::ControlFlow::Exit;
                    return;
                }
                _ => return,
            },
            glutin::event::Event::NewEvents(cause) => match cause {
                glutin::event::StartCause::ResumeTimeReached { .. } => (),
                glutin::event::StartCause::Init => (),
                _ => return,
            },
            _ => return,
        }

        let mut target = display.draw();
        scene.draw(&mut target);
        target.finish().unwrap();
    });
}
