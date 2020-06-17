extern crate image;

use image::{imageops, ImageBuffer, RgbImage};

fn main() {
    let mut img: RgbImage = ImageBuffer::new(256, 256);

    let red = image::Rgb([255, 0, 0]);
    let white = image::Rgb([255, 255, 255]);
    let green = image::Rgb([0, 255, 0]);

    draw_line(13, 20, 100, 60, &mut img, red);
    draw_line(20, 13, 40, 80, &mut img, white);
    draw_line(80, 40, 13, 20, &mut img, green);

    // flip vertically to make origin at bottom left
    imageops::flip_vertical_in_place(&mut img);
    img.save("output.bmp").unwrap();
}

fn draw_line(
    mut x1: i32,
    mut y1: i32,
    mut x2: i32,
    mut y2: i32,
    image: &mut RgbImage,
    color: image::Rgb<u8>,
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
    for x in x1..x2 {
        let t = (x - x1) as f32 / (x2 - x1) as f32;
        let y = y1 as f32 * t + ((1f32 - t) * y2 as f32);
        if steep {
            image.put_pixel(y as u32, x as u32, color);
        } else {
            image.put_pixel(x as u32, y as u32, color);
        }
    }
}
