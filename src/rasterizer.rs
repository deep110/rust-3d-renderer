use crate::utils;
use crate::mesh::SimplePolygon;
use cgmath::prelude::*;
use cgmath::{Vector2, Vector3};

#[allow(unused_imports)]
use test::Bencher;

const _WIDTH_F: f32 = (crate::WIDTH - 1) as f32;
const _HEIGHT_F: f32 = (crate::HEIGHT - 1) as f32;

/// Barycentric coordinates of a point are represented from points of triangle
/// itself For example: Given triangle with A, B, C, we can have a point P in
/// triangle written as:
///
/// P = (1 -u - v) A + u * B + v * C
///
/// P = A + u * AB + v * AC
///
/// So to find barycentric coordinates we just need to solve above equation for
/// u & v.
///
/// u ABx + v ACx + PAx = 0
///
/// u ABy + v ACy + PAy = 0
///
/// In vector terms, we are looking for a vector (u,v, 1) which is perpendicular
/// to both (ABx, ACx, PAx) and (ABy, ACy, PAy) i.e use cross product.
fn barycentric_coordinates(vertices: &[Vector3<f32>], point: &Vector3<f32>) -> Vector3<f32> {
    let u = Vector3::cross(
        Vector3::new(
            vertices[1].x - vertices[0].x,
            vertices[2].x - vertices[0].x,
            vertices[0].x - point.x,
        ),
        Vector3::new(
            vertices[1].y - vertices[0].y,
            vertices[2].y - vertices[0].y,
            vertices[0].y - point.y,
        ),
    );

    // so `abs(u[2])` < 1 means `u[2]` is 0, that means
    // triangle is degenerate, in this case return something with negative
    // coordinates
    if u[2].abs() < 1. {
        return Vector3::new(-1., 1., 1.);
    }
    return Vector3::new(
        1. - (u.x + u.y) as f32 / u.z as f32,
        u.y as f32 / u.z as f32,
        u.x as f32 / u.z as f32,
    );
}

fn render_triangle(vertices: &[Vector3<f32>], frame: &mut [u8], zbuffer: &mut [f32], color: &[u8]) {
    let mut bboxmin: Vector2<f32> = Vector2::new(f32::MAX, f32::MAX);
    let mut bboxmax: Vector2<f32> = Vector2::new(f32::MIN, f32::MIN);
    let clamp: Vector2<f32> = Vector2::new(_WIDTH_F, _HEIGHT_F);
    for i in 0..3 {
        for j in 0..2 {
            bboxmin[j] = f32::max(0., f32::min(bboxmin[j], vertices[i][j]));
            bboxmax[j] = f32::min(clamp[j], f32::max(bboxmax[j], vertices[i][j]));
        }
    }

    let mut point: Vector3<f32> = Vector3::new(0., 0., 0.);
    for i in bboxmin.x as i32..bboxmax.x as i32 + 1 {
        for j in bboxmin.y as i32..bboxmax.y as i32 + 1 {
            utils::update_vector2(&mut point, i, j);
            let bc_screen = barycentric_coordinates(vertices, &point);
            if bc_screen.x < 0. || bc_screen.y < 0. || bc_screen.z < 0. {
                continue;
            };
            point.z = 0.;

            // check with z buffer & then draw
            for i in 0..2 {
                point.z += vertices[i][2] * bc_screen[i]
            }
            let q = i as usize + (j * _WIDTH_F as i32) as usize;
            if zbuffer[q] < point.z {
                utils::set_pixel(i as usize, j as usize, frame, color);
                zbuffer[q] = point.z;
            }
        }
    }
}

#[bench]
fn bench_render_triangle(b: &mut Bencher) {
    let mut frame = [0u8; crate::WIDTH as usize * crate::HEIGHT as usize * 4];
    let mut zbuffer = [0f32; crate::WIDTH as usize * crate::HEIGHT as usize];
    let red = [255, 0, 0, 255];

    let pts = [
        Vector3::new(0., 0., 0.),
        Vector3::new(50., 0., 0.),
        Vector3::new(25., 25., 0.),
    ];

    b.iter(|| render_triangle(&pts, &mut frame, &mut zbuffer, &red));
}

fn world_to_screen(world_c: &Vector3<f32>) -> Vector3<f32> {
    Vector3::new(
        (world_c.x + 1.) * _WIDTH_F / 2.,
        (world_c.y + 1.) * _HEIGHT_F / 2.,
        world_c.z,
    )
}

pub fn rasterize_mesh(
    vertices: &Vec<Vector3<f32>>,
    faces: &Vec<SimplePolygon>,
    frame: &mut [u8],
    zbuffer: &mut [f32],
    light_dir: Vector3<f32>,
) {
    // each face is a triangle
    for face in faces {
        let mut world_coordinates: Vec<Vector3<f32>> = Vec::with_capacity(3);
        // coordinates of face triangles in screen coordinates
        let mut screen_coordinates: Vec<Vector3<f32>> = Vec::with_capacity(3);
        for i in 0..3 {
            // world coordinate of triangle vertex
            let tr_wc = vertices[face[i].0];
            screen_coordinates.push(world_to_screen(&tr_wc));
            world_coordinates.push(tr_wc);
        }
        // get normal vector to triangle and take dot product with light direction
        // to get intensity.
        let mut normal = Vector3::cross(
            world_coordinates[1] - world_coordinates[0],
            world_coordinates[2] - world_coordinates[0],
        );
        normal = normal.normalize();

        // use intensity as it is. Ignoring gamma correction
        let intensity = (normal.dot(light_dir) * 255.) as u8;
        if intensity > 0 {
            render_triangle(
                &screen_coordinates,
                frame,
                zbuffer,
                &[intensity, intensity, intensity, 255],
            );
        }
    }
}
