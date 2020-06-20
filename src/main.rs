#![feature(test)]
extern crate test;

mod renderer;
mod utils;

pub mod wavefront;
pub mod wireframe;

use pixels::{wgpu::Surface, Pixels, SurfaceTexture};
use std::env;
use wavefront::Obj;
use winit::dpi::LogicalSize;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;

// global variables
const WIDTH: u32 = 512;
const HEIGHT: u32 = 512;
const BLACK: [u8; 4] = [0, 0, 0, 255];

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
            .build(&event_loop)
            .unwrap()
    };

    let mut pixels = {
        let surface = Surface::create(&window);
        let surface_texture = SurfaceTexture::new(WIDTH, HEIGHT, surface);
        Pixels::new(WIDTH, HEIGHT, surface_texture).unwrap()
    };

    let mesh = init(&args[1]);

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
                // clear the frame buffer
                utils::clear(pixels.get_frame(), &BLACK);

                // redraw
                draw_mesh(&mesh, pixels.get_frame());

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

fn init(obj_path: &str) -> wavefront::ObjData {
    let mut mesh = Obj::load(obj_path).unwrap().data;
    mesh.normalize_vertices();

    return mesh;
}

fn draw_mesh(mesh: &wavefront::ObjData, frame: &mut [u8]) {
    for obj in &mesh.objects {
        for g in obj.groups.iter() {
            renderer::render_mesh(&mesh.position, &g.polys, frame);
        }
    }
}
