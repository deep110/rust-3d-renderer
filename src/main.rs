#![feature(test)]
extern crate test;

mod wireframe;

#[allow(dead_code)]
mod wavefront;

use image::{imageops, ImageBuffer, RgbImage};
use wavefront::Obj;

static WIDTH: u32 = 512;
static HEIGHT: u32 = 512;

fn main() {
    let head = Obj::load("assets/african_head.obj").unwrap().data;

    let mut img: RgbImage = ImageBuffer::new(WIDTH, HEIGHT);
    let white = image::Rgb([255, 255, 255]);

    for obj in head.objects {
        for g in obj.groups.iter() {
            wireframe::draw_object_wireframe(&head.position, &g.polys, &mut img, &white);
        }
    }

    // flip vertically to make origin at bottom left
    imageops::flip_vertical_in_place(&mut img);
    img.save("african_head.bmp").unwrap();
}
