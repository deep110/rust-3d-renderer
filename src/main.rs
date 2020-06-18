#![feature(test)]
extern crate test;
use test::Bencher;

use image::{imageops, ImageBuffer, RgbImage};
use obj::Obj;

static WIDTH: u32 = 511;
static HEIGHT: u32 = 511;

fn main() {
    let head = Obj::load("assets/african_head.obj").unwrap().data;

    let mut img: RgbImage = ImageBuffer::new(WIDTH+1, HEIGHT+1);
    let white = image::Rgb([255, 255, 255]);

    for obj in head.objects {
        for g in obj.groups.iter() {
            draw_object_wireframe(&head.position, &g.polys, &mut img, &white);
        }
    }

    // flip vertically to make origin at bottom left
    imageops::flip_vertical_in_place(&mut img);
    img.save("african_head.bmp").unwrap();
}

fn draw_object_wireframe(
    vertices: &Vec<[f32; 3]>,
    faces: &Vec<obj::SimplePolygon>,
    image: &mut RgbImage,
    color: &image::Rgb<u8>,
) {
    for face in faces {
        for i in 0..2 {
            let v0 = vertices[face.0[i].0];
            let v1 = vertices[face.0[(i + 1) % 3].0];

            let x0 = (v0[0] + 1f32) * WIDTH as f32 / 2.;
            let y0 = (v0[1] + 1f32) * HEIGHT as f32 / 2.;
            let x1 = (v1[0] + 1f32) * WIDTH as f32 / 2.;
            let y1 = (v1[1] + 1f32) * HEIGHT as f32 / 2.;

            draw_line(x0 as i32, y0 as i32, x1 as i32, y1 as i32, image, color);
        }
    }
}

fn draw_line(
    mut x1: i32,
    mut y1: i32,
    mut x2: i32,
    mut y2: i32,
    image: &mut RgbImage,
    color: &image::Rgb<u8>,
) {
    let mut steep = false;
    if ((x1 - x2) as i32).abs() < ((y1 - y2) as i32).abs() {
        std::mem::swap(&mut x1, &mut y1);
        std::mem::swap(&mut x2, &mut y2);
        steep = true;
    }
    if x1 > x2 {
        std::mem::swap(&mut x1, &mut x2);
        std::mem::swap(&mut y1, &mut y2);
    }
    let dx = x2 - x1;
    let dy = y2 - y1;
    let derror = (dy * 2).abs();
    let mut error = 0;
    let mut y = y1;
    for x in x1..x2 {
        if steep {
            image.put_pixel(y as u32, x as u32, *color);
        } else {
            image.put_pixel(x as u32, y as u32, *color);
        }
        error += derror;
        if error > dx {
            y += if y2 > y1 { 1 } else { -1 };
            error -= dx * 2;
        }
    }
}

#[bench]
fn bench_draw_line(b: &mut Bencher) {
    let mut img: RgbImage = ImageBuffer::new(256, 256);

    let red = image::Rgb([255, 0, 0]);
    b.iter(|| draw_line(30, 30, 10, 10, &mut img, &red));
}
