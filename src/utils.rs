const _WIDTH_C: usize = crate::WIDTH as usize;
const _HEIGHT_C: usize = crate::HEIGHT as usize - 1;

const A: i32 = 15342;
const C: i32 = 45194;
static mut prev: i32 = 0; // seed

// Sets the pixel color in frame buffer
// Also invert the y coordinate to make origin at bottom left corner
pub fn set_pixel(x: usize, y: usize, frame: &mut [u8], color: &[u8]) {
    let si = 4 * (x + _WIDTH_C * (_HEIGHT_C - y));
    frame[si..si + 4].copy_from_slice(color);
}

pub fn clear(frame: &mut [u8], color: &[u8]) {
    for pixel in frame.chunks_exact_mut(4) {
        pixel.copy_from_slice(color);
    }
}

pub fn get_random_color() -> [u8; 4] {
    [rand(255) as u8, rand(255) as u8, rand(255) as u8, 255]
}

fn rand(max: i32) -> i32 {
    unsafe {
        prev = (prev * A + C) % max;
        return prev;
    }
}
