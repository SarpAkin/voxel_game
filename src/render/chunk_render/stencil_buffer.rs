use std::sync::Arc;

use ash::vk;
use bytemuck::Pod;
use magma_renderer::core::*;

//Bump allocater
pub struct StencilBuffer {
    pub buffer: Buffer<u8>,
    top: u64,
}

impl StencilBuffer {
    pub fn new(core: &Arc<Core>, size: u32) -> eyre::Result<StencilBuffer> {
        Ok(StencilBuffer {
            buffer: core.create_buffer(vk::BufferUsageFlags::TRANSFER_SRC, size, true)?, //
            top: 0,
        })
    }

    pub fn upload(&mut self, bytes: &[u8]) -> Option<u64> {
        if self.top + bytes.len() as u64 > self.buffer.byte_size() {
            return None;
        }

        let data = self.buffer.get_data_mut().unwrap();
        let offset = self.top;

        data[(offset as usize)..][..bytes.len()].copy_from_slice(bytes);
        self.top += bytes.len() as u64;
        Some(offset)
    }

    pub fn allocate(&mut self, size: u64, allignment: u64) -> Option<(&mut [u8],u64)> {
        if self.top % allignment != 0 {
            self.top = (self.top / allignment + 1) * allignment;
        }

        if self.top + size as u64 > self.buffer.byte_size() {
            return None;
        }

        let offset = self.top;
        self.top += size;
        Some((&mut self.buffer.get_data_mut().unwrap()[(offset as usize)..((offset + size) as usize)],offset))
    }

    pub fn allocate_items<T: Pod>(&mut self, size: u64) -> Option<(&mut [T],u64)> {
        self.allocate(size * std::mem::size_of::<T>() as u64, std::mem::size_of::<T>() as u64)
            .map(|(s,o)| (bytemuck::cast_slice_mut(s),o))
    }

    pub fn reset(&mut self) { self.top = 0; }
}
