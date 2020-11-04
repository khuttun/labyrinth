use std::str::FromStr;

mod game;
mod graphics;
mod util;

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
    let window = winit::window::Window::new(&event_loop).expect("Failed to create window");

    let mut w = window.inner_size().width;
    let mut h = window.inner_size().height;

    if fullscreen {
        let monitor = window.available_monitors().next();
        w = monitor.as_ref().unwrap().size().width;
        h = monitor.as_ref().unwrap().size().height;
        window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(monitor)));
    }

    println!("Window size {} x {}", w, h);

    #[cfg(target_arch = "wasm32")]
    {
        use winit::platform::web::WindowExtWebSys;
        console_log::init().expect("could not initialize logger");
        std::panic::set_hook(Box::new(console_error_panic_hook::hook));
        // On wasm, append the canvas to the document body
        web_sys::window()
            .and_then(|win| win.document())
            .and_then(|doc| doc.body())
            .and_then(|body| {
                body.append_child(&web_sys::Element::from(window.canvas()))
                    .ok()
            })
            .expect("couldn't append canvas to document body");
    }

    window.set_cursor_visible(false);

    #[cfg(not(target_arch = "wasm32"))]
    {
        window
            .set_cursor_position(winit::dpi::PhysicalPosition::new(w / 2, h / 2))
            .expect("Failed center cursor");
        window.set_cursor_grab(true).expect("Failed to grab cursor");
    }

    // Create a level and set up the scene based on it
    #[cfg(target_arch = "wasm32")]
    let level1 = game::Level::from_json_str(include_str!("../level1.json"));
    #[cfg(not(target_arch = "wasm32"))]
    let level1 = game::Level::from_json_file("level1.json");

    #[cfg(not(target_arch = "wasm32"))]
    {
        let gfx = futures::executor::block_on(graphics::Instance::new(gfx_cfg, &window, w, h));
        util::play_level(
            level1,
            "level1_markings.png",
            gfx,
            event_loop,
            static_camera,
        );
    }
    #[cfg(target_arch = "wasm32")]
    {
        wasm_bindgen_futures::spawn_local(async move {
            let gfx = graphics::Instance::new(gfx_cfg, &window, w, h).await;
            util::play_level(
                level1,
                "level1_markings.png",
                gfx,
                event_loop,
                static_camera,
            );
        });
    }
}
