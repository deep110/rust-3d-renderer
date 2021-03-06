use cgmath::Vector3;

// Sets the pixel color in frame buffer
// Also invert the y coordinate to make origin at bottom left corner
pub fn set_pixel(x: usize, y: usize, frame: &mut [u8], color: &[u8], width: usize, height: usize) {
    let si = 4 * (x + (width + 1) * (height - y));
    frame[si..si + 4].copy_from_slice(color);
}

pub fn clear(frame: &mut [u8], color: &[u8]) {
    for pixel in frame.chunks_exact_mut(4) {
        pixel.copy_from_slice(color);
    }
}

pub fn update_vector2(v: &mut Vector3<f32>, x: i32, y: i32) {
    v.x = x as f32;
    v.y = y as f32;
}
