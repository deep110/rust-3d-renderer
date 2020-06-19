pub fn set_pixel(x: usize, y: usize, frame: &mut [u8], color: &[u8]) {
    let si = x + super::WIDTH as usize * y;
    frame[si..si + 4].copy_from_slice(color);
}

pub fn clear(frame: &mut [u8], color: &[u8]) {
    for pixel in frame.chunks_exact_mut(4) {
        pixel.copy_from_slice(color);
    }
}
