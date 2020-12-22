//! This library provides a light weight cross platform renderer pipeline
//!
//! It takes list of objects to render and directional light and fills up a
//! frame buffer.
//! It is mostly written for learning purposes and does not aim to be
//! replacement of any rendering library.
//!
//! Example

#![feature(test)]
extern crate test;

mod mesh;
mod rasterizer;
mod utils;
mod wireframe;

use cgmath::Vector3;
use mesh::{MeshData, MeshLoader};

const BLACK: [u8; 4] = [0, 0, 0, 255];

#[derive(Copy, Clone)]
pub struct Config<'a> {
    pub width: u32,
    pub height: u32,
    pub mesh_path: &'a str,
    pub light_direction: Vector3<f32>,
    pub is_wireframe: bool,
    pub default_color: [u8; 4],
}

pub struct RendererContext<'a> {
    config: Config<'a>,
    mesh: MeshData,
    zbuffer: Vec<f32>,
}

pub fn init<'a, 'b: 'a>(config: Config<'b>) -> RendererContext<'a> {
    let mut mesh = MeshLoader::load(config.mesh_path).unwrap().data;
    mesh.normalize_vertices();

    return RendererContext {
        config: config,
        mesh: mesh,
        zbuffer: vec![f32::MIN; (config.width * config.height) as usize],
    };
}

pub fn render_scene(rcontext: &mut RendererContext, frame_buffer: &mut [u8]) {
    let zbuffer = &mut rcontext.zbuffer[..];
    let mesh = &rcontext.mesh;
    let config = rcontext.config;

    // clear the frame buffer
    utils::clear(frame_buffer, &BLACK);

    for obj in &mesh.objects {
        for g in obj.groups.iter() {
            if config.is_wireframe {
                // show wireframe
                wireframe::draw_object_wireframe(&mesh.position, &g.polys, frame_buffer, &config);
            } else {
                // clear z buffer
                for i in 0..(config.width * config.height) as usize {
                    zbuffer[i] = f32::MIN;
                }
                rasterizer::rasterize_mesh(
                    &mesh.position,
                    &g.polys,
                    frame_buffer,
                    zbuffer,
                    &config,
                );
            }
        }
    }
}
