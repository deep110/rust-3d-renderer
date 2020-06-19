#![feature(test)]
extern crate test;

mod wireframe;

#[allow(dead_code)]
mod wavefront;

use std::env;
use image::{imageops, ImageBuffer, RgbImage};
use wavefront::Obj;

static WIDTH: u32 = 512;
static HEIGHT: u32 = 512;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("No obj file provided");
        return;
    }

    let mut mesh = Obj::load(&args[1]).unwrap().data;

    mesh.normalize_vertices();

    let mut img: RgbImage = ImageBuffer::new(WIDTH, HEIGHT);
    let white = image::Rgb([255, 255, 255]);

    for obj in mesh.objects {
        for g in obj.groups.iter() {
            wireframe::draw_object_wireframe(&mesh.position, &g.polys, &mut img, &white);
        }
    }

    // flip vertically to make origin at bottom left
    imageops::flip_vertical_in_place(&mut img);
    img.save("output.bmp").unwrap();
}
