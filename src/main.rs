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

    let w = display.gl_window().window().inner_size().width;
    let h = display.gl_window().window().inner_size().height;
    display.gl_window().window().set_cursor_position(
        glutin::dpi::PhysicalPosition::new(w / 2, h / 2)).unwrap();
    display.gl_window().window().set_cursor_visible(false);

    let level1 = game::Level::from_json("level1.json");
    println!("Level 1: {:#?}", level1);

    let quad = Rc::new(graphics::Shape::from_ply(&display, "quad.ply"));
    let cube = Rc::new(graphics::Shape::from_ply(&display, "cube.ply"));
    let sphere = Rc::new(graphics::Shape::from_ply(&display, "sphere.ply"));

    let mut board = graphics::Object::new(&quad);
    board.set_scaling(3.0, 2.0, 1.0);
    board.set_rotation(-std::f32::consts::PI / 2.0, 1.0, 0.0, 0.0);

    let mut wall = graphics::Object::new(&cube);
    wall.set_scaling(1.0, 3.0, 2.0);
    wall.set_rotation(std::f32::consts::PI / 4.0, 0.0, 1.0, 0.0);
    wall.set_position(0.0, 1.5, 0.0);

    let mut ball = graphics::Object::new(&sphere);
    ball.set_color(0.6, 0.4, 0.0);
    ball.set_shininess(100.0);
    ball.set_position(0.0, 0.0, 0.0);

    let mut scene = graphics::Scene::new(&display);
    scene.add_object(board);
    scene.add_object(wall);
    let ball_id = scene.add_object(ball);

    event_loop.run(move |event, _, control_flow| {
        let next_frame_time = std::time::Instant::now() + std::time::Duration::from_nanos(16_666_667);
        *control_flow = glutin::event_loop::ControlFlow::WaitUntil(next_frame_time);

        match event {
            glutin::event::Event::WindowEvent { event, .. } => match event {
                glutin::event::WindowEvent::CloseRequested => {
                    *control_flow = glutin::event_loop::ControlFlow::Exit;
                    return;
                },
                glutin::event::WindowEvent::CursorMoved { position, .. } => {
                    scene.get_object(ball_id).set_position(
                        5.0f32 * ((position.x as f32) - (w as f32) / 2.0f32) / ((w as f32) / 2.0f32),
                        0.0f32,
                        5.0f32 * ((position.y as f32) - (h as f32) / 2.0f32) / ((h as f32) / 2.0f32))
                },
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
