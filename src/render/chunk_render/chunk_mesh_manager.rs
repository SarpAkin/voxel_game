use std::{collections::HashMap, sync::Arc};

use ash::vk;
use bytemuck::{bytes_of, Pod, Zeroable};
use glam::Mat4;
use magma_renderer::core::*;
use magma_renderer::engine::material::*;

use crate::{
    game::{CameraData, Game},
    render::{renderpassmanager::RenderPassManager, VkDrawIndexedIndirectCommand},
};

use super::{
    primative_manager::{BatchUpload, PrimativeManager},
    stencil_buffer::StencilBuffer,
    ChunkMesh, Quad,
};

struct IDManager {
    id_counter: u32,
    free_ids: Vec<u32>,
}

impl IDManager {
    pub fn free_id(&mut self, id: u32) { self.free_ids.push(id); }

    pub fn new_id(&mut self) -> u32 {
        self.free_ids.pop().unwrap_or_else(|| {
            let old = self.id_counter;
            self.id_counter += 1;
            old
        })
    }

    pub fn new() -> IDManager { Self { id_counter: 0, free_ids: Vec::new() } }
}

enum ChunkUpdate {
    Removed,
    Inserted,
}

pub struct ChunkMeshManager {
    chunk_ids: HashMap<[i32; 3], u32>,
    id_man: IDManager,
    id_cap: u32,
    chunk_buffer: Buffer<ChunkGPUBufferData>,
    opaque_meshes: PrimativeManager,
    stencil_buffers: Box<[StencilBuffer]>,
    queued_meshes: Vec<ChunkMesh>,
    updated_chunks: HashMap<[i32; 3], ChunkUpdate>,
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod, Debug, Default)]
pub struct ChunkGPUBufferData {
    pos: [i32; 3],
    flags: u32,
}

impl ChunkMeshManager {
    //gettres
    pub fn get_max_chunk_id(&self) -> u32 {self.id_man.id_counter}
    pub fn get_chunk_buffer(&self) -> &Buffer<ChunkGPUBufferData> {&self.chunk_buffer}
    pub fn get_opaque_meshes(&self) -> &PrimativeManager {&self.opaque_meshes}

    pub fn submit_meshes(&mut self, mut meshes: Vec<ChunkMesh>) { self.queued_meshes.append(&mut meshes); }

    pub fn flush_stencil(&mut self, cmd: &mut CommandBuffer, frame_index: usize) {
        let mut quad_mesh_uploads = Vec::new();

        let stencil = &mut self.stencil_buffers[frame_index];
        stencil.reset();

        let mut remaining_chunks = 100;

        while let Some(mesh) = self.queued_meshes.pop() {
            let chunk_id = *self.chunk_ids.entry(mesh.pos).or_insert_with(|| {
                self.updated_chunks.insert(mesh.pos, ChunkUpdate::Inserted);
                self.id_man.new_id()
            });

            let Some(byte_offset) = stencil.upload(bytemuck::cast_slice(mesh.quads.as_slice())) else {
                self.queued_meshes.push(mesh);
                break
            };

            quad_mesh_uploads.push(BatchUpload {
                byte_offset: byte_offset as u32,
                primative_count: mesh.quads.len() as u32,
                id: chunk_id,
            });

            remaining_chunks -= 1;
            if remaining_chunks <= 0 {
                break;
            }
        }

        self.opaque_meshes.insert_batches(cmd, &stencil.buffer, quad_mesh_uploads);
        self.opaque_meshes.sweep_and_flush(cmd, stencil);

        let mut copy_commands = Vec::new();

        for (pos, update) in &self.updated_chunks {
            let chunk_id = *self.chunk_ids.get(pos).unwrap();
            let gpu_chunk = ChunkGPUBufferData {
                pos: *pos,
                flags: match update {
                    ChunkUpdate::Removed => 0x0,
                    ChunkUpdate::Inserted => 0x1,
                },
            };
            const CHUNK_GPU_SIZE: u64 = std::mem::size_of::<ChunkGPUBufferData>() as u64;
            let offset = stencil.upload(bytes_of(&gpu_chunk)).unwrap();
            copy_commands.push(vk::BufferCopy {
                src_offset: offset,
                dst_offset: chunk_id as u64 * CHUNK_GPU_SIZE,
                size: CHUNK_GPU_SIZE,
            });
        }

        self.updated_chunks.clear();

        unsafe {
            cmd.copy_buffer_reigons(stencil.buffer.inner(), self.chunk_buffer.inner(), &copy_commands);
        }
    }

    pub fn total_batch_count(&self) -> u32 { self.opaque_meshes.batch_count() }

    pub fn new(core: &Arc<Core>) -> eyre::Result<Self> {
        let cap = 100_000;

        Ok(Self {
            chunk_ids: HashMap::new(),
            id_man: IDManager::new(),
            id_cap: cap,
            chunk_buffer: core.create_buffer(
                vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
                cap,
                false,
            )?,
            opaque_meshes: PrimativeManager::new(
                core,
                std::mem::size_of::<Quad>() as u32,
                128_000,
                cap,
                vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::VERTEX_BUFFER | vk::BufferUsageFlags::STORAGE_BUFFER,
            )?,
            stencil_buffers: (0..2).map(|_| StencilBuffer::new(core, 10_000_000)).collect::<eyre::Result<_>>()?,
            queued_meshes: Vec::new(),
            updated_chunks: HashMap::new(),
        })
    }
}