mod rasterizer;
mod wireframe;

use crate::mesh;
use crate::Config;

pub fn render_object(
    mesh: &mesh::MeshData,
    config: &Config,
    frame_buffer: &mut [u8],
    zbuffer: &mut [f32],
) {
    for obj in &mesh.objects {
        for g in obj.groups.iter() {
            if config.is_wireframe {
                // show wireframe
                wireframe::draw_mesh_wireframe(&mesh.position, &g.polys, frame_buffer, config);
            } else {
                rasterizer::rasterize_mesh(&mesh.position, &g.polys, frame_buffer, zbuffer, config);
            }
        }
    }
}
