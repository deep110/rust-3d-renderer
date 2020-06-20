#![feature(test)]
extern crate test;

mod rasterizer;
mod utils;

pub mod wavefront;
pub mod wireframe;

use cgmath::Vector3;
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
            .build(&event_loop)
            .unwrap()
    };

    let mut pixels = {
        let surface = Surface::create(&window);
        let surface_texture = SurfaceTexture::new(WIDTH, HEIGHT, surface);
        Pixels::new(WIDTH, HEIGHT, surface_texture).unwrap()
    };

    let mut zbuffer = [f32::MIN; (WIDTH * HEIGHT) as usize];

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
                rasterize_scene(&mesh, pixels.get_frame(), &mut zbuffer);

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

// right now scene is just one obj mesh
fn rasterize_scene(mesh: &wavefront::ObjData, frame_buffer: &mut [u8], zbuffer: &mut [f32]) {
    // clear z buffer
    for i in 0..(WIDTH * HEIGHT) as usize {
        zbuffer[i] = f32::MIN;
    }
    for obj in &mesh.objects {
        for g in obj.groups.iter() {
            rasterizer::rasterize_mesh(&mesh.position, &g.polys, frame_buffer, zbuffer);
        }
    }
}
