const _WIDTH_C: usize = crate::WIDTH as usize;
const _HEIGHT_C: usize = crate::HEIGHT as usize - 1;

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
