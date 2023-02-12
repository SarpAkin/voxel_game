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
    pub data: [u32; 2],
}

pub struct ChunkMesher {}

impl ChunkMesher {
    fn ambient_occulusion_corner(side0: bool, side1: bool, corner: bool) -> u32 {
        //false = opaque
        //true = transparent

        if !side0 && !side1 {
            return 0;
        }

        return side0 as u32 + side1 as u32 + corner as u32;
    }

    fn ambient_occulusion_face(&self, x: i32, y: i32, z: i32, direction: Direction, view: &ChunkView) -> (u32, bool) {
        let mut block_n_ = false;
        let mut block_ne = false;
        let mut block_nw = false;
        let mut block_s_ = false;
        let mut block_se = false;
        let mut block_sw = false;
        #[allow(non_snake_case)]
        let mut block__e = false;
        #[allow(non_snake_case)]
        let mut block__w = false;

        match direction {
            Direction::YP | Direction::YN => {
                let base_y = y + match direction {
                    Direction::YP => 1,
                    Direction::YN => -1,
                    _ => unreachable!(),
                };

                block_n_ = view.get_tile(x, base_y, z + 1).properties().is_transparent;
                block_ne = view.get_tile(x + 1, base_y, z + 1).properties().is_transparent;
                block_nw = view.get_tile(x - 1, base_y, z + 1).properties().is_transparent;
                block_s_ = view.get_tile(x, base_y, z - 1).properties().is_transparent;
                block_se = view.get_tile(x + 1, base_y, z - 1).properties().is_transparent;
                block_sw = view.get_tile(x - 1, base_y, z - 1).properties().is_transparent;
                block__e = view.get_tile(x + 1, base_y, z).properties().is_transparent;
                block__w = view.get_tile(x - 1, base_y, z).properties().is_transparent;
            }
            Direction::XP | Direction::XN => {
                let base_x = x + match direction {
                    Direction::XP => 1,
                    Direction::XN => -1,
                    _ => unreachable!(),
                };

                block_n_ = view.get_tile(base_x, y, z + 1).properties().is_transparent;
                block_ne = view.get_tile(base_x, y + 1, z + 1).properties().is_transparent;
                block_nw = view.get_tile(base_x, y - 1, z + 1).properties().is_transparent;
                block_s_ = view.get_tile(base_x, y, z - 1).properties().is_transparent;
                block_se = view.get_tile(base_x, y + 1, z - 1).properties().is_transparent;
                block_sw = view.get_tile(base_x, y - 1, z - 1).properties().is_transparent;
                block__e = view.get_tile(base_x, y + 1, z).properties().is_transparent;
                block__w = view.get_tile(base_x, y - 1, z).properties().is_transparent;
            }
            Direction::ZP | Direction::ZN => {
                let base_z = z + match direction {
                    Direction::ZP => 1,
                    Direction::ZN => -1,
                    _ => unreachable!(),
                };

                block_n_ = view.get_tile(x, y + 1, base_z).properties().is_transparent;
                block_ne = view.get_tile(x + 1, y + 1, base_z).properties().is_transparent;
                block_nw = view.get_tile(x - 1, y + 1, base_z).properties().is_transparent;
                block_s_ = view.get_tile(x, y - 1, base_z).properties().is_transparent;
                block_se = view.get_tile(x + 1, y - 1, base_z).properties().is_transparent;
                block_sw = view.get_tile(x - 1, y - 1, base_z).properties().is_transparent;
                block__e = view.get_tile(x + 1, y, base_z).properties().is_transparent;
                block__w = view.get_tile(x - 1, y, base_z).properties().is_transparent;
            }
            _ => {}
        }

        let mut corner_0 = Self::ambient_occulusion_corner(block_s_, block__w, block_sw);
        let mut corner_1 = Self::ambient_occulusion_corner(block_n_, block__w, block_nw);
        let mut corner_2 = Self::ambient_occulusion_corner(block_s_, block__e, block_se);
        let mut corner_3 = Self::ambient_occulusion_corner(block_n_, block__e, block_ne);

        match direction {
            Direction::XP | Direction::ZP | Direction::YN => {
                (corner_1,corner_2) = (corner_2,corner_1);
            },

            _ => {},
        }


        (corner_0 | (corner_1 << 2) | (corner_2 << 4) | (corner_3 << 6), corner_0 + corner_3 > corner_1 + corner_2)
    }

    fn new_quad(&self, tile: Tile, x: i32, y: i32, z: i32, direction: Direction, view: &ChunkView) -> Quad {
        let mut data_0 = 0u32;

        assert!(x < 32);
        assert!(y < 32);
        assert!(z < 32);

        data_0 |= (x as u32) | ((y as u32) << 5) | ((z as u32) << 10); // position

        let facing_bits = match direction {
            Direction::XP => 0,
            Direction::XN => 1,
            Direction::YP => 2,
            Direction::YN => 3,
            Direction::ZP => 4,
            Direction::ZN => 5,
        };

        data_0 |= facing_bits << 15; //facing direction

        data_0 |= (tile.0 as u32) << 18; // material

        let mut data_1 = 0;

        let (ao_bits, ao_flip_flag) = self.ambient_occulusion_face(x, y, z, direction, view);
        data_1 |= ao_bits;
        data_1 |= (ao_flip_flag as u32) << 31;

        Quad { data: [data_0, data_1] }
    }

    pub fn mesh_chunk(&self, voxelworld: &VoxelWorld, chunkpos: &[i32; 3]) -> ChunkMesh {
        let [cx, cy, cz] = *chunkpos;

        let mut view = voxelworld.get_chunk_view([cx - 1, cy - 1, cz - 1], [cx + 1, cy + 1, cz + 1]);
        view.offsets.iter_mut().for_each(|n| *n += CHUNK_SIZE as i32);

        let mut quads: Vec<Quad> = Vec::new();

        for y in 0..32 {
            for z in 0..32 {
                for x in 0..32 {
                    let tile = view.get_tile(x, y, z);

                    if tile == Tile(0) {
                        continue;
                    }

                    let ypt = view.get_tile(x, y + 1, z);
                    let ynt = view.get_tile(x, y - 1, z);
                    let xpt = view.get_tile(x + 1, y, z);
                    let xnt = view.get_tile(x - 1, y, z);
                    let zpt = view.get_tile(x, y, z + 1);
                    let znt = view.get_tile(x, y, z - 1);

                    #[rustfmt::skip] if ypt.transparent() { quads.push(self.new_quad(tile, x, y, z, Direction::YP,&view))};
                    #[rustfmt::skip] if ynt.transparent() { quads.push(self.new_quad(tile, x, y, z, Direction::YN,&view))};
                    #[rustfmt::skip] if xpt.transparent() { quads.push(self.new_quad(tile, x, y, z, Direction::XP,&view))};
                    #[rustfmt::skip] if xnt.transparent() { quads.push(self.new_quad(tile, x, y, z, Direction::XN,&view))};
                    #[rustfmt::skip] if zpt.transparent() { quads.push(self.new_quad(tile, x, y, z, Direction::ZP,&view))};
                    #[rustfmt::skip] if znt.transparent() { quads.push(self.new_quad(tile, x, y, z, Direction::ZN,&view))};
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
