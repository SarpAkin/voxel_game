use std::sync::Arc;

use crate::game::FrameIndex;

use super::*;

use ash::vk;
use magma_renderer::core::CommandBuffer;
use specs::prelude::*;

/* Quad Storage 2x32 bits

    // u32-0
    x y z 5x3 bits 0-15
    facing direction 3 bits 15-18
    material 14 bits 18-32

    // u32-1
    // reserved for future

*/

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
pub struct Quad {
    // pub verticies: [ChunkVertex; 4],
    pub data: [u32;2],
}

pub struct ChunkMesher {}

impl ChunkMesher {
    // fn new_quad(&self, tile: Tile, sx: u32, sy: u32, sz: u32, ex: u32, ey: u32, ez: u32) -> Quad { todo!() }
    fn new_quad(&self, tile: Tile, x: usize, y: usize, z: usize, facing: Facing) -> Quad {
        let mut data_0 = 0u32;

        assert!(x < 32);
        assert!(y < 32);
        assert!(z < 32);

        data_0 |= (x as u32) | ((y as u32) << 5) | ((z as u32) << 10); // position

        let facing_bits = match facing {
            Facing::XP => 0,
            Facing::XN => 1,
            Facing::YP => 2,
            Facing::YN => 3,
            Facing::ZP => 4,
            Facing::ZN => 5,
        };

        data_0 |= facing_bits << 15; //facing direction

        data_0 |= (tile.0 as u32) << 18; // material

        Quad { data: [data_0,0] }
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
            .filter_map(|(chunk, _, _entity)| {
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
