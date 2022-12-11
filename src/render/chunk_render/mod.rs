use crate::game::{voxels::*, Game};
use bytemuck::{Pod, Zeroable};
use specs::prelude::*;
use magma_renderer::{auto_description, core::Renderpass};

pub mod mesher;
pub mod renderer;

auto_description!(
    #[repr(C)]
    #[derive(Clone, Copy, Zeroable, Pod)]
    pub struct ChunkVertex {
        pos: [f32; 3],
        uv: [f32; 2],
    }
);

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
pub struct Quad {
    verticies: [ChunkVertex; 4],
}

pub enum Facing {
    XP,
    XN,
    YP,
    YN,
    ZP,
    ZN,
}

pub fn init(game: &mut Game, renderpass: &dyn Renderpass) {
    game.world.register::<renderer::ChunkMesh>();

    renderer::register_render_data(game, renderpass).unwrap();

    game.insert_frame_task(Box::new(move |_w, d| {
        d.add(mesher::ChunkMesher {}, "mesh chunks", &[]);
        d.add(renderer::Renderer {}, "render chunks", &["mesh chunks"]);
    }))
}
