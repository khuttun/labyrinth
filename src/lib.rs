use mobile_entry_point::mobile_entry_point;
use std::rc::Rc;
use std::str::FromStr;

mod game;
mod game_loop;
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
    // TODO: Could this be handled by polling for window properties or waiting for winit::event::Event::Resumed?
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
        // on Android, the first Resumed event will set the window
        #[cfg(not(target_os = "android"))]
        gfx.set_window(Some(&window));
        play(gfx, event_loop, window, static_camera, stats);
    }
    #[cfg(target_arch = "wasm32")]
    {
        wasm_bindgen_futures::spawn_local(async move {
            let mut gfx = graphics::Instance::new(gfx_cfg, w, h).await;
            gfx.set_window(Some(&window));
            play(gfx, event_loop, window, static_camera, stats);
        });
    }
}

fn play(
    gfx: graphics::Instance,
    event_loop: winit::event_loop::EventLoop<()>,
    window: winit::window::Window,
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
    let game = game::Game::new(level);

    // Enter the main loop
    let mut gl = game_loop::GameLoop::new(
        window,
        game,
        gfx,
        scene,
        board_id,
        ball_id,
        static_camera,
        stats,
    );
    event_loop.run(move |ev, _, cf| *cf = gl.handle_event(&ev));
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

// Create an image suitable for texture use from raw image file bytes
fn create_image(bytes: &[u8], format: image::ImageFormat) -> image::RgbaImage {
    image::load_from_memory_with_format(bytes, format)
        .unwrap()
        .flipv()
        .into_rgba8()
}
