use cgmath::Vector3;
use pixels::{wgpu::Surface, Pixels, SurfaceTexture};
use std::env;
use winit::dpi::LogicalSize;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;

use toy_renderer::Config;

// global variables
const WIDTH: u32 = 512;
const HEIGHT: u32 = 512;
const WHITE: [u8; 4] = [255, 255, 255, 255];
const LIGHT_DIR: Vector3<f32> = Vector3::new(0., 0., 1.);

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("No obj file provided");
        return;
    }

    let event_loop = EventLoop::new();
    let window = {
        let size = LogicalSize::new(WIDTH as f64, HEIGHT as f64);
        WindowBuilder::new()
            .with_title("Toy Renderer")
            .with_inner_size(size)
            .with_resizable(false)
            .with_always_on_top(true)
            .build(&event_loop)
            .unwrap()
    };

    let mut pixels = {
        let surface = Surface::create(&window);
        let surface_texture = SurfaceTexture::new(WIDTH, HEIGHT, surface);
        Pixels::new(WIDTH, HEIGHT, surface_texture).unwrap()
    };

    let file_path = Box::leak(args[1].clone().into_boxed_str());
    let config = Config {
        width: WIDTH,
        height: HEIGHT,
        mesh_path: file_path,
        is_wireframe: false,
        light_direction: LIGHT_DIR,
        default_color: WHITE,
    };

    let mut rcontext = toy_renderer::init(config);

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
            } if window_id == window.id() => *control_flow = ControlFlow::Exit,

            Event::WindowEvent {
                event: WindowEvent::Resized(size),
                window_id: _,
            } => {
                pixels.resize(size.width, size.height);
            }

            Event::MainEventsCleared => {
                window.request_redraw();
            }

            Event::RedrawRequested(_) => {
                // redraw
                toy_renderer::render_scene(&mut rcontext, pixels.get_frame());

                if pixels
                    .render()
                    .map_err(|e| println!("pixels.render() failed: {}", e))
                    .is_err()
                {
                    *control_flow = ControlFlow::Exit;
                    return;
                }
            }

            _ => (),
        }
    });
}
