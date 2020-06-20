use crate::utils;
use crate::wavefront::SimplePolygon;
use cgmath::prelude::*;
use cgmath::{Vector2, Vector3};

#[allow(unused_imports)]
use test::Bencher;

/// Barycentric coordinates of a point are represented from points of triangle
/// itself For example: Given triangle with A, B, C, we can have a point P in
/// triangle written as:
///
/// P = (1 -u - v) A + u * B + v * C
///
/// P = A + u * AB + v * AC
///
/// So to find barycentric cooridinates we just need to solve above equation for
/// u & v.
///
/// u ABx + v ACx + PAx = 0
///
/// u ABy + v ACy + PAy = 0
///
/// In vector terms, we are looking for a vector (u,v, 1) which is perpendicular
/// to both (ABx, ACx, PAx) and (ABy, ACy, PAy) i.e use cross product.
fn barycentric_coordinates(
    vertices: &[Vector2<isize>],
    point_x: isize,
    point_y: isize,
) -> Vector3<f32> {
    let u = Vector3::cross(
        Vector3::new(
            vertices[1].x - vertices[0].x,
            vertices[2].x - vertices[0].x,
            vertices[0].x - point_x,
        ),
        Vector3::new(
            vertices[1].y - vertices[0].y,
            vertices[2].y - vertices[0].y,
            vertices[0].y - point_y,
        ),
    );

    // so `abs(u[2])` < 1 means `u[2]` is 0, that means
    // triangle is degenerate, in this case return something with negative
    // coordinates
    if u[2].abs() < 1 {
        return Vector3::new(-1., 1., 1.);
    }
    return Vector3::new(
        1. - (u.x + u.y) as f32 / u.z as f32,
        u.y as f32 / u.z as f32,
        u.x as f32 / u.z as f32,
    );
}

pub fn render_triangle(vertices: &[Vector2<isize>], frame: &mut [u8], color: &[u8]) {
    let mut bboxmin: Vector2<isize> =
        Vector2::new((crate::WIDTH - 1) as isize, (crate::HEIGHT - 1) as isize);
    let mut bboxmax: Vector2<isize> = Vector2::new(0, 0);
    let clamp: Vector2<isize> = Vector2::new(bboxmin.x, bboxmin.y);
    for i in 0..3 {
        for j in 0..2 {
            bboxmin[j] = isize::max(0, isize::min(bboxmin[j], vertices[i][j]));
            bboxmax[j] = isize::min(clamp[j], isize::max(bboxmax[j], vertices[i][j]));
        }
    }

    for i in bboxmin.x..bboxmax.x {
        for j in bboxmin.y..bboxmax.y {
            let bc_screen = barycentric_coordinates(vertices, i, j);
            if bc_screen.x < 0. || bc_screen.y < 0. || bc_screen.z < 0. {
                continue;
            };
            utils::set_pixel(i as usize, j as usize, frame, color);
        }
    }
}

#[bench]
fn bench_render_traingle(b: &mut Bencher) {
    let mut frame = [0u8; crate::WIDTH as usize * crate::HEIGHT as usize * 4];
    let red = [255, 0, 0, 255];

    let pts = [
        Vector2::new(0, 0),
        Vector2::new(50, 0),
        Vector2::new(25, 25),
    ];

    b.iter(|| render_triangle(&pts, &mut frame, &red));
}

pub fn render_mesh(vertices: &Vec<Vector3<f32>>, faces: &Vec<SimplePolygon>, frame: &mut [u8]) {
    let width: f32 = (crate::WIDTH - 1) as f32;
    let height: f32 = (crate::HEIGHT - 1) as f32;

    // each face is a triangle
    for face in faces {
        // coordinates of face triangles in screen coordinates
        let mut world_coordinates: Vec<Vector3<f32>> = Vec::with_capacity(3);
        let mut screen_coordinates: Vec<Vector2<isize>> = Vec::with_capacity(3);
        for i in 0..3 {
            // world coordinate of triangle vertex
            let tr_wc = vertices[face[i].0];
            screen_coordinates.push(Vector2::new(
                ((tr_wc.x + 1.) * width / 2.) as isize,
                ((tr_wc.y + 1.) * height / 2.) as isize,
            ));
            world_coordinates.push(tr_wc);
        }
        // get normal vector to triangle and take dot product with light direction
        // to get intensity. Ignoring gamma correction
        let mut normal = Vector3::cross(
            world_coordinates[1] - world_coordinates[0],
            world_coordinates[2] - world_coordinates[0],
        );
        normal = normal.normalize();

        let intensity = (normal.dot(crate::LIGHT_DIR) * 255.) as u8;
        if intensity > 0 {
            render_triangle(
                &screen_coordinates,
                frame,
                &[intensity, intensity, intensity, 255],
            );
        }
    }
}
