mod graphics;

fn main() {
    let event_loop = glium::glutin::event_loop::EventLoop::new();
    let wb = glium::glutin::window::WindowBuilder::new();
    let cb = glium::glutin::ContextBuilder::new().with_depth_buffer(24);
    let display = glium::Display::new(wb, cb, &event_loop).unwrap();

    let quad = graphics::Shape::from_ply(&display, "quad.ply");
    let sphere = graphics::Shape::from_ply(&display, "sphere.ply");

    let mut board = graphics::Object::new(&quad);
    board.set_scaling(3.0, 2.0, 1.0);
    board.set_rotation(-std::f32::consts::PI / 2.0, 1.0, 0.0, 0.0);

    let mut ball = graphics::Object::new(&sphere);
    ball.set_position(-1.0, 2.0, 1.0);
}
