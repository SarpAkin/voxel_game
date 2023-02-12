use crate::game::{voxels::*, Game};
use bytemuck::{Pod, Zeroable};
use magma_renderer::{auto_description, core::Renderpass};
use specs::prelude::*;

mod chunk_renderer;
pub mod mesher;
mod primative_manager;
mod stencil_buffer;
mod chunk_mesh_manager;

auto_description!(
    #[repr(C)]
    #[derive(Clone, Copy, Zeroable, Pod)]
    pub struct ChunkVertex {
        pos: [f32; 3],
        uv: [f32; 2],
    }
);

pub use mesher::Quad;

pub enum Direction {
    XP,
    XN,
    YP,
    YN,
    ZP,
    ZN,
}

pub fn init(game: &mut Game, renderpass: &dyn Renderpass) {
    game.insert_frame_task(Box::new(move |_w, d| {
        d.add(mesher::ChunkMesher {}, "chunk mesh", &[]);
    }));

    chunk_renderer::register_render_data(game).unwrap();
}

pub struct ChunkMesh {
    pos: [i32; 3],
    quads: Vec<Quad>,
}
