#[macro_use]
extern crate glium;
use glium::glutin;
use std::rc::Rc;
mod graphics;

fn main() {
    let event_loop = glium::glutin::event_loop::EventLoop::new();
    let wb = glium::glutin::window::WindowBuilder::new();
    let cb = glium::glutin::ContextBuilder::new().with_depth_buffer(24).with_multisampling(2);
    let display = glium::Display::new(wb, cb, &event_loop).unwrap();

    let quad = Rc::new(graphics::Shape::from_ply(&display, "quad.ply"));
    let sphere = Rc::new(graphics::Shape::from_ply(&display, "sphere.ply"));

    let mut board = graphics::Object::new(&quad);
    board.set_scaling(3.0, 2.0, 1.0);
    board.set_rotation(-std::f32::consts::PI / 2.0, 1.0, 0.0, 0.0);

    let mut ball = graphics::Object::new(&sphere);
    ball.set_color(0.6, 0.4, 0.0);
    ball.set_shininess(100.0);
    ball.set_position(-1.0, 2.0, 1.0);

    let mut scene = graphics::Scene::new(&display);
    scene.add_object(board);
    scene.add_object(ball);

    event_loop.run(move |event, _, control_flow| {
        let next_frame_time = std::time::Instant::now() + std::time::Duration::from_nanos(16_666_667);
        *control_flow = glutin::event_loop::ControlFlow::WaitUntil(next_frame_time);

        match event {
            glutin::event::Event::WindowEvent { event, .. } => match event {
                glutin::event::WindowEvent::CloseRequested => {
                    *control_flow = glutin::event_loop::ControlFlow::Exit;
                    return;
                },
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
