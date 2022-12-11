use std::{collections::HashMap, ops::Add, sync::Mutex};

use nalgebra::Vector3;
use specs::prelude::*;

use crate::game::Game;

use super::{worldgen::WorldGen, *};

pub const CHUNK_SIZE: usize = 32;
pub const CHUNK_AREA: usize = CHUNK_SIZE * CHUNK_SIZE;
pub const CHUNK_VOLUME: usize = CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE;

pub fn chunk_to_world_pos([cx, cy, cz]: [i32; 3]) -> Vector3<f32> {
    Vector3::new((cx * CHUNK_SIZE as i32) as f32, (cy * CHUNK_SIZE as i32) as f32, (cz * CHUNK_SIZE as i32) as f32)
}

pub struct ChunkComponent {
    pub chunkpos: [i32; 3],
}

#[derive(Default)]
pub struct ModifiedChunk;
impl Component for ModifiedChunk {
    type Storage = NullStorage<Self>;
}

impl ChunkComponent {
    pub fn get_chunk_ref<'a>(&self, voxel_world: &'a VoxelWorld) -> ChunkRef<'a> {
        voxel_world.get_chunk(&self.chunkpos).unwrap()
    }

    pub fn get_chunk_ref_mut<'a>(&self, voxel_world: &'a mut VoxelWorld) -> ChunkRefMut<'a> {
        voxel_world.get_chunk_mut(&self.chunkpos).unwrap()
    }
}

impl Component for ChunkComponent {
    type Storage = DenseVecStorage<Self>;
}

pub struct ChunkRef<'a> {
    voxel_ref: &'a [Tile; CHUNK_VOLUME],
    cpos: [i32; 3],
}

impl<'a> ChunkRef<'a> {
    pub fn get_block(&self, x: usize, y: usize, z: usize) -> Tile {
        assert!(x < CHUNK_SIZE && y < CHUNK_SIZE && z < CHUNK_SIZE);

        self.voxel_ref[x + z * CHUNK_SIZE + y * (CHUNK_SIZE * CHUNK_SIZE)]
    }

    pub fn chunk_pos(&self) -> [i32; 3] { self.cpos }
    pub fn world_pos(&self) -> Vector3<f32> { chunk_to_world_pos(self.cpos) }

    pub fn empty() -> ChunkRef<'static> { ChunkRef { voxel_ref: &empty_voxels, cpos: [i32::MIN; 3] } }
}

pub struct ChunkRefMut<'a> {
    voxel_ref: &'a mut [Tile; CHUNK_VOLUME],
}

impl<'a> ChunkRefMut<'a> {
    pub fn get_block(&mut self, x: usize, y: usize, z: usize) -> &mut Tile {
        assert!(x < CHUNK_SIZE && y < CHUNK_SIZE && z < CHUNK_SIZE);

        &mut self.voxel_ref[x + z * CHUNK_SIZE + y * (CHUNK_SIZE * CHUNK_SIZE)]
    }
}

#[derive(Debug)]
pub struct VoxelWorld {
    chunk_voxels: HashMap<[i32; 3], Box<[Tile; CHUNK_VOLUME]>>,
}

impl VoxelWorld {
    pub fn new() -> VoxelWorld { Self { chunk_voxels: HashMap::new() } }

    pub fn get_chunk(&self, pos: &[i32; 3]) -> Option<ChunkRef> {
        self.chunk_voxels.get(pos).and_then(|c| Some(ChunkRef { voxel_ref: c, cpos: *pos }))
    }

    pub fn get_chunk_mut(&mut self, pos: &[i32; 3]) -> Option<ChunkRefMut> {
        self.chunk_voxels.get_mut(pos).and_then(|c| Some(ChunkRefMut { voxel_ref: c }))
    }

    pub fn register_chunk(&mut self, pos: &[i32; 3], voxels: Box<[Tile; CHUNK_VOLUME]>) {
        self.chunk_voxels.insert(*pos, voxels);
    }

    pub fn remove_chunk(&mut self, pos: &[i32; 3]) -> Option<Box<[Tile; CHUNK_VOLUME]>> { self.chunk_voxels.remove(pos) }
}

const empty_voxels: [Tile; CHUNK_VOLUME] = [Tile(0); CHUNK_VOLUME];

pub fn init(game: &mut Game) {
    game.world.register::<ChunkComponent>();
    game.world.register::<ModifiedChunk>();
    game.world.insert(VoxelWorld::new());

    game.world.create_entity().with(ChunkComponent { chunkpos: [0, 0, 0] }).with(ModifiedChunk).build();

    let mut worldgen = WorldGen::new(374437);
    for x in -10..11 {
        for z in -10..11 {
            if x * x + z * z <= 100 {
                for y in 0..3 {
                    worldgen.queue_chunk([x, y, z]);
                }
            }
        }
    }

    // worldgen.queue_chunk([1, 0, 1]);
    // worldgen.queue_chunk([1, 0, 0]);
    // worldgen.queue_chunk([0, 0, 0]);

    game.world.insert(Mutex::new(worldgen));

    game.insert_frame_task(Box::new(|w, d| {
        d.add_thread_local(ClearModified {});

        let chunks = w.fetch_mut::<Mutex<WorldGen>>().lock().unwrap().receive_chunks();
        for c in &chunks {
            w.create_entity().with(ChunkComponent { chunkpos: c.pos }).with(ModifiedChunk).build();
        }

        let mut vw = w.fetch_mut::<VoxelWorld>();
        for c in chunks {
            vw.register_chunk(&c.pos, c.voxels);
        }
    }));
}

struct ClearModified;

impl<'a> System<'a> for ClearModified {
    type SystemData = WriteStorage<'a, ModifiedChunk>;

    fn run(&mut self, mut data: Self::SystemData) { data.clear(); }
}
