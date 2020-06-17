extern crate image;

use image::{ImageBuffer, RgbImage};

fn main() {
    // Construct a new RGB ImageBuffer with the specified width and height.
    let mut img: RgbImage = ImageBuffer::new(512, 512);

    // Put a pixel at coordinate (100, 100).
    img.put_pixel(100, 100, image::Rgb([255, 0, 0]));

    img.save("output.bmp").unwrap();
}
