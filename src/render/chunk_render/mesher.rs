use std::sync::Arc;

use crate::game::FrameIndex;

use super::*;

use ash::vk;
use magma_renderer::core::CommandBuffer;
use specs::prelude::*;

pub struct ChunkMesher {}

impl ChunkMesher {
    // fn new_quad(&self, tile: Tile, sx: u32, sy: u32, sz: u32, ex: u32, ey: u32, ez: u32) -> Quad { todo!() }
    fn new_quad(&self, tile: Tile, x: usize, y: usize, z: usize, facing: Facing) -> Quad {
        let color = match facing {
            Facing::XP => 0xFF,
            Facing::XN => 0xFF,
            Facing::YP => 0xFF_00,
            Facing::YN => 0xFF_00,
            Facing::ZP => 0xFF_00_00,
            Facing::ZN => 0xFF_00_00,
        };

        let mx = x as f32;
        let my = y as f32;
        let mz = z as f32;

        let base_cord = tile.0 as f32 / 16.0;
        let tile_size = 1.0 / 16.0;

        let uvs = [
            [base_cord, 0.0], //
            [base_cord + tile_size, 0.0],
            [base_cord, 1.0],
            [base_cord + tile_size, 1.0],
        ];

        let verticies = match facing {
            Facing::XP => [
                ChunkVertex { pos: [mx + 1.0, my - 0.0, mz - 0.0], uv: uvs[0] },
                ChunkVertex { pos: [mx + 1.0, my + 1.0, mz - 0.0], uv: uvs[1] },
                ChunkVertex { pos: [mx + 1.0, my - 0.0, mz + 1.0], uv: uvs[2] },
                ChunkVertex { pos: [mx + 1.0, my + 1.0, mz + 1.0], uv: uvs[3] },
            ],
            Facing::XN => [
                ChunkVertex { pos: [mx - 0.0, my - 0.0, mz - 0.0], uv: uvs[0] },
                ChunkVertex { pos: [mx - 0.0, my - 0.0, mz + 1.0], uv: uvs[1] },
                ChunkVertex { pos: [mx - 0.0, my + 1.0, mz - 0.0], uv: uvs[2] },
                ChunkVertex { pos: [mx - 0.0, my + 1.0, mz + 1.0], uv: uvs[3] },
            ],
            Facing::YP => [
                ChunkVertex { pos: [mx - 0.0, my + 1.0, mz - 0.0], uv: uvs[0] },
                ChunkVertex { pos: [mx - 0.0, my + 1.0, mz + 1.0], uv: uvs[1] },
                ChunkVertex { pos: [mx + 1.0, my + 1.0, mz - 0.0], uv: uvs[2] },
                ChunkVertex { pos: [mx + 1.0, my + 1.0, mz + 1.0], uv: uvs[3] },
            ],
            Facing::YN => [
                ChunkVertex { pos: [mx - 0.0, my - 0.0, mz - 0.0], uv: uvs[0] },
                ChunkVertex { pos: [mx + 1.0, my - 0.0, mz - 0.0], uv: uvs[1] },
                ChunkVertex { pos: [mx - 0.0, my - 0.0, mz + 1.0], uv: uvs[2] },
                ChunkVertex { pos: [mx + 1.0, my - 0.0, mz + 1.0], uv: uvs[3] },
            ],
            Facing::ZP => [
                ChunkVertex { pos: [mx - 0.0, my - 0.0, mz + 1.0], uv: uvs[0] },
                ChunkVertex { pos: [mx + 1.0, my - 0.0, mz + 1.0], uv: uvs[1] },
                ChunkVertex { pos: [mx - 0.0, my + 1.0, mz + 1.0], uv: uvs[2] },
                ChunkVertex { pos: [mx + 1.0, my + 1.0, mz + 1.0], uv: uvs[3] },
            ],
            Facing::ZN => [
                ChunkVertex { pos: [mx - 0.0, my - 0.0, mz - 0.0], uv: uvs[0] },
                ChunkVertex { pos: [mx - 0.0, my + 1.0, mz - 0.0], uv: uvs[1] },
                ChunkVertex { pos: [mx + 1.0, my - 0.0, mz - 0.0], uv: uvs[2] },
                ChunkVertex { pos: [mx + 1.0, my + 1.0, mz - 0.0], uv: uvs[3] },
            ],
        };

        Quad { verticies }
    }

    pub fn mesh_chunk(&self, voxelworld: &VoxelWorld, chunkpos: &[i32; 3]) -> ChunkMesh {
        let [cx, cy, cz] = *chunkpos;

        let chunk = voxelworld.get_chunk(chunkpos).unwrap_or(ChunkRef::empty());
        let ypchunk = voxelworld.get_chunk(&[cx, cy + 1, cz]).unwrap_or(ChunkRef::empty());
        let ynchunk = voxelworld.get_chunk(&[cx, cy - 1, cz]).unwrap_or(ChunkRef::empty());
        let xpchunk = voxelworld.get_chunk(&[cx + 1, cy, cz]).unwrap_or(ChunkRef::empty());
        let xnchunk = voxelworld.get_chunk(&[cx - 1, cy, cz]).unwrap_or(ChunkRef::empty());
        let zpchunk = voxelworld.get_chunk(&[cx, cy, cz + 1]).unwrap_or(ChunkRef::empty());
        let znchunk = voxelworld.get_chunk(&[cx, cy, cz - 1]).unwrap_or(ChunkRef::empty());

        let mut quads: Vec<Quad> = Vec::new();

        const CHUNKSIZEM1: usize = CHUNK_SIZE - 1;

        for y in 0..32 {
            for z in 0..32 {
                for x in 0..32 {
                    let tile = chunk.get_block(x, y, z);

                    if tile == Tile(0) {
                        continue;
                    }

                    let (ypt, ynt) = match y {
                        0 => (chunk.get_block(x, y + 1, z), ynchunk.get_block(x, CHUNKSIZEM1, z)),
                        CHUNKSIZEM1 => (ypchunk.get_block(x, 0, z), chunk.get_block(x, y - 1, z)),
                        _ => (chunk.get_block(x, y + 1, z), chunk.get_block(x, y - 1, z)),
                    };

                    let (xpt, xnt) = match x {
                        0 => (chunk.get_block(x + 1, y, z), xnchunk.get_block(CHUNKSIZEM1, y, z)),
                        CHUNKSIZEM1 => (xpchunk.get_block(0, y, z), chunk.get_block(x - 1, y, z)),
                        _ => (chunk.get_block(x + 1, y, z), chunk.get_block(x - 1, y, z)),
                    };

                    let (zpt, znt) = match z {
                        0 => (chunk.get_block(x, y, z + 1), znchunk.get_block(x, y, CHUNKSIZEM1)),
                        CHUNKSIZEM1 => (zpchunk.get_block(x, y, 0), chunk.get_block(x, y, z - 1)),
                        _ => (chunk.get_block(x, y, z + 1), chunk.get_block(x, y, z - 1)),
                    };

                    #[rustfmt::skip] if ypt.transparent() { quads.push(self.new_quad(tile, x, y, z, Facing::YP))};
                    #[rustfmt::skip] if ynt.transparent() { quads.push(self.new_quad(tile, x, y, z, Facing::YN))};
                    #[rustfmt::skip] if xpt.transparent() { quads.push(self.new_quad(tile, x, y, z, Facing::XP))};
                    #[rustfmt::skip] if xnt.transparent() { quads.push(self.new_quad(tile, x, y, z, Facing::XN))};
                    #[rustfmt::skip] if zpt.transparent() { quads.push(self.new_quad(tile, x, y, z, Facing::ZP))};
                    #[rustfmt::skip] if znt.transparent() { quads.push(self.new_quad(tile, x, y, z, Facing::ZN))};
                }
            }
        }

        ChunkMesh { pos: *chunkpos, quads }
    }
}

impl ChunkMesh {
    fn empty(&self) -> bool { self.quads.len() == 0 }
}

impl<'a> System<'a> for ChunkMesher {
    type SystemData = (
        ReadExpect<'a, VoxelWorld>,
        ReadExpect<'a, crate::render::renderpassmanager::RenderPassManager>,
        ReadStorage<'a, ChunkComponent>,
        ReadStorage<'a, ModifiedChunk>,
        WriteExpect<'a, super::chunk_mesh_manager::ChunkMeshManager>,
        ReadExpect<'a, FrameIndex>,
        Entities<'a>,
    );

    fn run(&mut self, (vworld, rpman, chunk, modifiedf, mut mesh_man, frame_index, entities): Self::SystemData) {
        // let mut expired_meshes = Vec::new();
        let mut cmd = CommandBuffer::new_secondry(rpman.core());
        cmd.begin_secondry(None).unwrap();

        let meshes = (&chunk, &modifiedf, &entities)
            .par_join()
            .filter_map(|(chunk, _, entity)| {
                let mesh = self.mesh_chunk(&*vworld, &chunk.chunkpos);
                if mesh.empty() {
                    return None;
                }
                Some(mesh)
            })
            .collect();

        mesh_man.submit_meshes(meshes);
        mesh_man.flush_stencil(&mut cmd, frame_index.index());

        // cmd.add_dependency(&Arc::new(expired_meshes));
        cmd.end().unwrap();
        rpman.submit_compute(cmd);
    }
}
