use ash::vk;
use magma_renderer::core::*;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use super::stencil_buffer::StencilBuffer;
struct PrimativeBatch {
    count: u32,
    offset: u32,
    id: u32,
    pool_id: u32,
}

impl PrimativeBatch {
    pub fn compact(&self) -> [u32; 2] { [self.offset, (self.pool_id << 24) | (self.count & 0xFF_FFFF)] }
}

struct PrimativeReigon {
    primative_offset: u32,
    cap: u32,
    top: u32, //relative to offset
    usage: u32,
    batch_ids: HashSet<u32>,
    is_written: bool,
}

pub struct PrimativePool {
    primative_size: u32,
    pool_id: u32,
    primative_buffer: Buffer<u8>,
    reigons: Vec<PrimativeReigon>,
    reigon_size: u32,
}

impl PrimativePool {
    pub fn get_primative_buffer(&self) -> &Buffer<u8> { &self.primative_buffer }
    pub fn get_batch_count(&self) -> u32 { self.reigons.iter().map(|r| r.batch_ids.len() as u32).sum() }
}

pub struct PrimativeManager {
    primative_size: u32,
    pools: Vec<PrimativePool>,
    batch_description_buffer: Buffer<u8>,
    batches: HashMap<u32, PrimativeBatch>,
    max_id: u32,
    reigon_size: u32,
    core: Arc<Core>,
    buffer_usage: vk::BufferUsageFlags,
    updated_batches: HashSet<u32>,
}

pub struct BatchUpload {
    pub byte_offset: u32,
    pub primative_count: u32,
    pub id: u32,
}

impl PrimativeManager {
    // getters
    pub fn get_batch_descriptions(&self) -> &Buffer<u8> { &self.batch_description_buffer }
    pub fn batch_count(&self) -> u32 { self.batches.len() as u32 }
    pub fn get_pools(&self) -> &[PrimativePool] { &self.pools }
    
    pub fn new(
        core: &Arc<Core>,
        primative_size: u32,
        reigon_size: u32,
        max_id: u32,
        buffer_usage: vk::BufferUsageFlags,
    ) -> eyre::Result<PrimativeManager> {
        Ok(Self {
            primative_size,
            pools: Vec::new(),
            batch_description_buffer: core.create_buffer(
                vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
                max_id * 8,
                false,
            )?,
            batches: HashMap::new(),
            max_id,
            reigon_size,
            core: core.clone(),
            buffer_usage,
            updated_batches: HashSet::new(),
        })
    }

    pub fn insert_batches(&mut self, cmd: &mut CommandBuffer, stencil_buffer: &Buffer<u8>, uploads: Vec<BatchUpload>) {
        uploads.iter().for_each(|u| self.remove_batch_from_reigon(u.id));
        let mut i = 0;

        while i < uploads.len() {
            let mut reigons: Vec<_> = self
                .pools
                .iter()
                .flat_map(|p| p.reigons.iter().enumerate().map(|(rid, r)| (p.pool_id, rid as u32, r.score())))
                .filter(|(_, _, score)| *score > 0.5)
                .collect();

            if reigons.len() == 0 {
                self.new_pool().unwrap();
                continue;
            };

            reigons.sort_unstable_by(|a, b| a.2.total_cmp(&b.2).reverse());

            self.insert_into_reigons(cmd, &stencil_buffer, &uploads, &reigons, &mut i);
        }

        // self.update_gpu_batches(cmd, ids, stencil_buffer)

        uploads.iter().for_each(|u| self.update_batch_gpu(u.id));
    }

    pub fn remove_batches(&mut self, ids: &[u32]) {
        for id in ids {
            self.remove_batch_from_reigon(*id);
            self.batches.remove(id);
            self.update_batch_gpu(*id);
        }
    }

    pub fn sweep_and_flush(&mut self, cmd: &mut CommandBuffer, stencil_buffer: &mut StencilBuffer) {
        self.sweep(cmd);

        self.update_gpu_batches(cmd, stencil_buffer);

        for pool in &mut self.pools {
            for reigon in &mut pool.reigons {
                reigon.is_written = false; //set it back to false for the next frame
            }
        }
    }

    fn sweep(&mut self, cmd: &mut CommandBuffer) {
        let mut fragmented_reigons: Vec<_> = self
            .pools
            .iter()
            .flat_map(|p| p.reigons.iter().enumerate().map(|(rid, r)| (p.pool_id, rid as u32, r.fragmentation_score())))
            .filter(|(_, _, score)| *score > 0.15)
            .filter(|(p, r, _)| self.pools[*p as usize].reigons[*r as usize].is_written == false) //check if it is written this frame
            .collect();

        if fragmented_reigons.len() == 0 {
            return;
        }

        fragmented_reigons.sort_unstable_by(|a, b| a.2.total_cmp(&b.2).reverse());

        fragmented_reigons.shrink_to(4); //we want to sweep at max 4 reigon

        let mut dst_reigons: Vec<_> = self
            .pools
            .iter()
            .flat_map(|p| p.reigons.iter().enumerate().map(|(rid, r)| (p.pool_id, rid as u32, r.fragmentation_score())))
            .filter(|(_, _, score)| *score < 0.08)
            .collect();

        if dst_reigons.len() == 0 {
            return;
        }

        dst_reigons.sort_unstable_by(|a, b| a.2.total_cmp(&b.2));

        let mut dst_reigon_index = 0;

        for (poolid, reigonid, _) in &dst_reigons {
            let mut batches = self.pools[*poolid as usize].reigons[*reigonid as usize].reset_and_get_batches();

            while dst_reigon_index < dst_reigons.len() {
                let (dst_pool_id, dst_reigon_id, _) = dst_reigons[dst_reigon_index];
                self.sweep_batches_to_reigon(cmd, *poolid, &mut batches, dst_pool_id, dst_reigon_id);
                if batches.len() > 0 {
                    dst_reigon_index += 1;
                } else {
                    break;
                }
            }

            //if we are out of clean reigons handle the remaining batches and break.
            if dst_reigon_index >= dst_reigons.len() {
                self.handle_remaining_batches(cmd, *poolid, batches);
                break;
            }
        }
    }

    fn handle_remaining_batches(&mut self, cmd: &mut CommandBuffer, src_pool_id: u32, mut batches: Vec<u32>) {
        self.new_pool().unwrap();
        self.sweep_batches_to_reigon(cmd, src_pool_id, &mut batches, (self.pools.len() - 1) as u32, 0);
    }

    fn sweep_batches_to_reigon(
        &mut self,
        cmd: &mut CommandBuffer,
        src_pool_id: u32,
        batches: &mut Vec<u32>,
        dst_pool_id: u32,
        dst_reigon_id: u32,
    ) {
        let dst_pool = self.pools.last_mut().unwrap();
        let dst_reigon = &mut dst_pool.reigons[dst_reigon_id as usize];

        let mut copy_commands = Vec::new();

        while let Some(batch_id) = batches.pop() {
            let Some(batch) = self.batches.get_mut(&batch_id) else {
                eprintln!("batch_id: {batch_id} is not found. skipping it in sweep");
                continue;
            };

            let Some(new_offset) = dst_reigon.allocate(batch.count) else {
                batches.push(batch_id);//push batch id back in order to prevent leakage
                break;
            };

            dst_reigon.batch_ids.insert(batch_id);
            copy_commands.push(vk::BufferCopy {
                src_offset: batch.offset as u64 * self.primative_size as u64,
                dst_offset: new_offset as u64 * self.primative_size as u64,
                size: new_offset as u64 * self.primative_size as u64,
            });

            batch.pool_id = dst_pool.pool_id;
            batch.offset = new_offset;

            // set batch to updated
            self.updated_batches.insert(batch_id);
        }

        unsafe {
            cmd.copy_buffer_reigons(
                self.pools[src_pool_id as usize].primative_buffer.inner(),
                self.pools[dst_pool_id as usize].primative_buffer.inner(),
                &copy_commands,
            );
        }
    }

    fn remove_batch_from_reigon(&mut self, id: u32) {
        if let Some(batch) = self.batches.get(&id) {
            let pool = &mut self.pools[batch.pool_id as usize];
            let reigon = &mut pool.reigons[(batch.offset / pool.reigon_size) as usize];
            reigon.batch_ids.remove(&batch.id);
        }
    }

    fn update_batch_gpu(&mut self, id: u32) { self.updated_batches.insert(id); }

    fn update_gpu_batches(&mut self, cmd: &mut CommandBuffer, stencil_buffer: &mut StencilBuffer) {
        let (gpu_data, byte_offset) = stencil_buffer.allocate_items(self.updated_batches.len() as u64).unwrap();

        let mut copies = Vec::with_capacity(self.updated_batches.len());

        let compacted_size = 8;

        for (i, id) in self.updated_batches.iter().enumerate() {
            gpu_data[i] = self.batches.get(id).map_or([0, 0], |b| b.compact());
            copies.push(vk::BufferCopy {
                src_offset: i as u64 * compacted_size + byte_offset,
                dst_offset: *id as u64 * compacted_size,
                size: compacted_size,
            });
        }

        unsafe {
            cmd.copy_buffer_reigons(stencil_buffer.buffer.inner(), self.batch_description_buffer.inner(), &copies);
        }

        self.updated_batches.clear();
    }

    fn insert_into_reigons(
        &mut self,
        cmd: &mut CommandBuffer,
        stencil_buffer: &Buffer<u8>,
        uploads: &[BatchUpload],
        reigons: &[(u32, u32, f32)],
        i: &mut usize,
    ) {
        for (pid, rid, _) in reigons {
            if *i >= uploads.len() {
                break;
            }

            let pool = &mut self.pools[*pid as usize];
            let reigon = &mut pool.reigons[*rid as usize];
            reigon.is_written = true;

            let mut copy_cmds: Vec<vk::BufferCopy> = Vec::new();

            while let Some(upload) = uploads.get(*i) {
                let Some(alloc_offset) = reigon.allocate(upload.primative_count) else {break};

                copy_cmds.push(vk::BufferCopy {
                    src_offset: upload.byte_offset as u64,
                    dst_offset: (alloc_offset * self.primative_size) as u64,
                    size: (upload.primative_count * self.primative_size) as u64,
                });
                self.batches.insert(
                    upload.id,
                    PrimativeBatch { count: upload.primative_count, offset: alloc_offset, id: upload.id, pool_id: *pid },
                );
                reigon.batch_ids.insert(upload.id);

                *i += 1;
            }

            unsafe {
                cmd.copy_buffer_reigons(stencil_buffer.inner(), pool.primative_buffer.inner(), &copy_cmds);
            }
        }
    }

    fn new_pool(&mut self) -> eyre::Result<()> {
        let reigon_count = self.pools.last().map(|p| p.reigons.len() as u32).unwrap_or(4);

        self.pools.push(PrimativePool {
            primative_size: self.primative_size,
            pool_id: self.pools.len() as u32,
            primative_buffer: self.core.create_buffer(
                self.buffer_usage,
                self.primative_size * self.reigon_size * reigon_count,
                false,
            )?,
            reigons: (0..reigon_count)
                .map(|i| PrimativeReigon {
                    primative_offset: i * self.reigon_size,
                    cap: self.reigon_size,
                    top: 0,
                    batch_ids: HashSet::new(),
                    usage: 0,
                    is_written: false,
                })
                .collect(),
            reigon_size: self.reigon_size,
        });

        Ok(())
    }
}

impl PrimativeReigon {
    fn score(&self) -> f32 {
        let top = self.top as f32;
        let cap = self.cap as f32;
        let usage = self.usage as f32;

        let empty_percent = 1.0 - (top / cap);
        if usage == top {
            return empty_percent;
        }

        (usage / top) * empty_percent
    }

    fn fragmentation_score(&self) -> f32 { (self.top - self.usage) as f32 / self.cap as f32 }

    fn allocate(&mut self, batch_size: u32) -> Option<u32> {
        if batch_size + self.top <= self.cap {
            let allocation = self.primative_offset + self.top;
            self.top += batch_size;
            self.usage += batch_size;
            Some(allocation)
        } else {
            None
        }
    }

    fn reset_and_get_batches(&mut self) -> Vec<u32> {
        self.top = 0;
        self.usage = 0;

        let mut vec = Vec::new();
        vec.extend(self.batch_ids.drain());
        vec
    }
}
