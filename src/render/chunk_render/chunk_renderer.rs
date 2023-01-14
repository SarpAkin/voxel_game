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
    chunk_mesh_manager::ChunkMeshManager,
    primative_manager::{BatchUpload, PrimativeManager},
    stencil_buffer::StencilBuffer,
    ChunkMesh, Quad,
};

//cpu side
struct FramelyData {
    draw_offset_buffer: Buffer<u32>,
}

pub struct ChunkRenderManager {
    indirect_draw_buffer: Buffer<VkDrawIndexedIndirectCommand>,
    draw_count_buffer: Buffer<u32>,
    // draw_parameter_buffer: Buffer<ChunkGPUBufferData>,
    framely_data: Box<[FramelyData]>,
    proj_view: Mat4,
    standart_opaque_material: MaterialID,
    shared_data: Arc<ChunkRenderSharedData>,
    core: Arc<Core>,
}

// struct RenderTypeMaterial<'a>{
//     pipeline:&'a Arc<Pipeline>,
//     dset:vk::DescriptorSet,
// }

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct PushConstant {
    multi_draw_offset: u32,
}

pub struct ChunkRenderSharedData {
    quad_index_buffer: Buffer<u16>,
    // chunk_data_set_layout: vk::DescriptorSetLayout,
    // quad_buffer_set_layout: vk::DescriptorSetLayout,
    cull_pipeline: Arc<Pipeline>,
}

impl ChunkRenderManager {
    pub fn set_material(&mut self, primative_id: u32, material_id: MaterialID) {
        self.standart_opaque_material = material_id;
    }

    pub fn cull_and_draw_chunks(
        &mut self,
        mesh_manager: &ChunkMeshManager,
        material_manager: &MaterialManager,
        draw_cmd: &mut CommandBuffer,
        compute_cmd: &mut CommandBuffer,
        descriptor_pool: &mut DescriptorPool,
        frame_index: usize,
        cam_data: &CameraData,
    ) -> eyre::Result<()> {
        if mesh_manager.total_batch_count() > self.indirect_draw_buffer.size() {
            //extend the capcacity of indirect draw buffers and parameter buffers
            let new_buffer = self.core.create_buffer(
                self.indirect_draw_buffer.get_usage(),
                (self.indirect_draw_buffer.size() * 2).max(mesh_manager.total_batch_count()),
                false,
            )?;

            compute_cmd.add_dependency(&Arc::new(std::mem::replace(&mut self.indirect_draw_buffer, new_buffer)));

            // println!("resized indirect draw buffer to {}",self.indirect_draw_buffer.size());
        }
        self.proj_view = cam_data.proj_view;

        let material = material_manager.get_material(self.standart_opaque_material).unwrap();
        draw_cmd.bind_material(&material);

        draw_cmd.bind_descriptor_set(0, cam_data.dset);
        draw_cmd.bind_index_buffer(self.shared_data.quad_index_buffer.as_slice());

        let chunk_data_set = DescriptorSetBuilder::new()
            .add_ssbo(&[mesh_manager.get_chunk_buffer()])
            .build(material.pipeline().get_descriptor_set_layout(1).unwrap(), descriptor_pool)?;
        draw_cmd.bind_descriptor_set(1, chunk_data_set);

        let mut draw_counter = 0;
        self.draw_and_cull_mesh_type(
            &mesh_manager.get_opaque_meshes(),
            &material,
            draw_cmd,
            0,
            frame_index,
            &mut draw_counter,
            descriptor_pool,
        )?;

        //culling
        compute_cmd.bind_pipeline(&self.shared_data.cull_pipeline);

        let cull_set = DescriptorSetBuilder::new()
            .add_ssbo(&[mesh_manager.get_chunk_buffer()])
            .add_ssbo(&[mesh_manager.get_opaque_meshes().get_batch_descriptions()])
            .add_ssbo(&[&self.indirect_draw_buffer])
            // .add_ssbo(&[&self.draw_parameter_buffer])
            .add_ssbo(&[&self.draw_count_buffer])
            .add_ssbo(&[&self.framely_data[frame_index].draw_offset_buffer])
            .build(self.shared_data.cull_pipeline.get_descriptor_set_layout(0).unwrap(), descriptor_pool)?;
        compute_cmd.bind_descriptor_set(0, cull_set);

        #[repr(C)]
        #[derive(Debug, Pod, Clone, Copy, Zeroable)]
        struct CullPush {
            chunk_count: u32,
        }

        compute_cmd.push_constant(
            &CullPush { chunk_count: mesh_manager.get_max_chunk_id() },
            vk::ShaderStageFlags::COMPUTE,
            0,
        );

        let chunk_count = mesh_manager.get_max_chunk_id() as u32;
        let group_size = 128;

        if chunk_count == 0 {
            return Ok(());
        }

        unsafe {
            compute_cmd.device().cmd_fill_buffer(
                compute_cmd.inner(),
                self.draw_count_buffer.inner(),
                0,
                self.draw_count_buffer.byte_size(),
                0,
            );
            compute_cmd.dispatch((chunk_count - 1) / group_size + 1, 1, 1);

            compute_cmd.pipeline_barrier(
                vk::PipelineStageFlags::COMPUTE_SHADER,
                vk::PipelineStageFlags::DRAW_INDIRECT,
                vk::DependencyFlags::DEVICE_GROUP,
                &[],
                &[
                    vk::BufferMemoryBarrier::builder()
                        .src_access_mask(vk::AccessFlags::SHADER_WRITE)
                        .dst_access_mask(vk::AccessFlags::INDIRECT_COMMAND_READ)
                        .buffer(self.indirect_draw_buffer.inner())
                        .size(vk::WHOLE_SIZE)
                        .build(),
                    vk::BufferMemoryBarrier::builder()
                        .src_access_mask(vk::AccessFlags::SHADER_WRITE)
                        .dst_access_mask(vk::AccessFlags::INDIRECT_COMMAND_READ)
                        .buffer(self.draw_count_buffer.inner())
                        .size(vk::WHOLE_SIZE)
                        .build(),
                ],
                &[],
            );
        }

        Ok(())
    }

    pub fn new_shared_data(
        core: &Arc<Core>,
        material_manager: &mut MaterialManager,
    ) -> eyre::Result<Arc<ChunkRenderSharedData>> {
        //
        let mut cmd = core.new_cmd();
        cmd.begin()?;

        let shared_data = ChunkRenderSharedData {
            quad_index_buffer: cmd.gpu_buffer_from_data(
                &(0..16384)
                    .flat_map(|i| [i * 4 + 0, i * 4 + 1, i * 4 + 2, i * 4 + 2, i * 4 + 1, i * 4 + 3])
                    .collect::<Vec<u16>>(),
                vk::BufferUsageFlags::INDEX_BUFFER,
            )?,
            cull_pipeline: material_manager.compile_compute_shader("res/chunk_cull.comp")?.0,
        };

        cmd.end()?;
        cmd.immediate_submit()?;

        Ok(Arc::new(shared_data))
    }

    fn draw_and_cull_mesh_type(
        &mut self,
        primative_man: &PrimativeManager,
        material: &Material,
        draw_cmd: &mut CommandBuffer,
        primative_id: u32,
        frame_index: usize,
        draw_counter: &mut u32,
        descriptor_pool: &mut DescriptorPool,
    ) -> eyre::Result<()> {
        //bind materials
        draw_cmd.bind_material(material);

        for (i, pool) in primative_man.get_pools().iter().enumerate() {
            let multi_draw_index = primative_id * 256 + i as u32;
            let multi_draw_count = pool.get_batch_count();
            let multi_draw_offset = *draw_counter;
            *draw_counter += multi_draw_count;

            self.framely_data[frame_index].draw_offset_buffer.get_data_mut().unwrap()[multi_draw_index as usize] =
                multi_draw_offset;

            //bind pool set and push constant
            // let push = PushConstant { multi_draw_offset };
            // draw_cmd.push_constant(&push, vk::ShaderStageFlags::VERTEX, 0);

            if false {
                let set = DescriptorSetBuilder::new()
                    .add_ssbo(&[pool.get_primative_buffer()])
                    .build(material.pipeline().get_descriptor_set_layout(3).unwrap(), descriptor_pool)?;
                draw_cmd.bind_descriptor_set(3, set);
            } else {
                draw_cmd.bind_vertex_buffers(&[pool.get_primative_buffer()])
            }

            //draw
            unsafe {
                let stride = 20;
                draw_cmd.draw_indexed_indirect_count(
                    self.indirect_draw_buffer.inner(),
                    multi_draw_offset as u64 * stride as u64, //byte offset
                    self.draw_count_buffer.inner(),
                    multi_draw_index as u64 * 4 as u64, //byte offset
                    multi_draw_count,
                    stride,
                );
            }
        }

        Ok(())
    }

    pub(crate) fn new(core: &Arc<Core>, material_manager: &mut MaterialManager) -> eyre::Result<ChunkRenderManager> {
        let shared_data = Self::new_shared_data(core, material_manager)?;
        let draw_counter_count = 1 * 256;

        Ok(Self {
            core: core.clone(),
            indirect_draw_buffer: core.create_buffer(
                vk::BufferUsageFlags::INDIRECT_BUFFER | vk::BufferUsageFlags::STORAGE_BUFFER,
                250,
                false,
            )?,
            draw_count_buffer: core.create_buffer(
                vk::BufferUsageFlags::STORAGE_BUFFER
                    | vk::BufferUsageFlags::TRANSFER_DST
                    | vk::BufferUsageFlags::INDIRECT_BUFFER,
                draw_counter_count,
                false,
            )?,
            // draw_parameter_buffer: core.create_buffer(
            //     vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
            //     initial_chunk_limit,
            //     false,
            // )?,
            framely_data: (0..2)
                .map(|_| {
                    Ok(FramelyData {
                        draw_offset_buffer: core.create_buffer(
                            vk::BufferUsageFlags::STORAGE_BUFFER,
                            draw_counter_count,
                            true,
                        )?,
                    })
                })
                .collect::<eyre::Result<_>>()?,
            proj_view: glam::Mat4::IDENTITY,
            shared_data,
            standart_opaque_material: MaterialID::NULL,
        })
    }
}

mod render_system {
    use std::{borrow::BorrowMut, ops::Deref};

    use crate::{
        game::FrameIndex,
        render::{chunk_render::ChunkVertex, renderpassmanager::RenderPassManager},
    };

    use super::*;
    use specs::prelude::*;

    pub struct ChunkRendererData {
        pub render_managers: HashMap<&'static str, ChunkRenderManager>,
    }

    impl ChunkRendererData {
        pub fn new(
            core: &Arc<Core>,
            renderpass_manager: &RenderPassManager,
            material_manager: &mut MaterialManager,
        ) -> eyre::Result<ChunkRendererData> {
            let mut d = Self { render_managers: HashMap::new() };

            material_manager.set_vertex_layout("chunk_vertex".into(), ChunkVertex::get_desciption());

            let mut render_manager = ChunkRenderManager::new(core, material_manager).unwrap();

            let mut cmd = core.new_cmd();
            cmd.begin()?;
            let gpass = renderpass_manager.get_subpass("gpass").unwrap();

            render_manager.set_material(
                0,
                material_manager.load_material(
                    &mut cmd,
                    "res/chunk.mat.yaml".into(),
                )?,
            );

            cmd.end()?;
            cmd.immediate_submit()?;

            d.render_managers.insert("gpass", render_manager);

            Ok(d)
        }
    }

    pub struct ChunkRenderer;

    impl<'a> System<'a> for ChunkRenderer {
        type SystemData = (
            WriteExpect<'a, ChunkRendererData>,
            ReadExpect<'a, crate::render::renderpassmanager::RenderPassManager>,
            ReadExpect<'a, crate::game::RenderGlobals>,
            ReadExpect<'a, crate::game::CameraData>,
            ReadExpect<'a, ChunkMeshManager>,
            ReadExpect<'a, MaterialManager>,
            ReadExpect<'a, FrameIndex>,
        );

        fn run(
            &mut self,
            (mut render_data, rp_man, gloabls, cam_data, mesh_manager, mat_man, frame_index): Self::SystemData,
        ) {
            let gpass = rp_man.get_subpass("gpass").unwrap();
            let mut draw_cmd = gpass.new_cmd().unwrap();
            let mut ccmd = gloabls.core().new_secondry_cmd();
            ccmd.begin_secondry(None).unwrap();

            let mut descriptor_pool = gloabls.frame_data().descriptor_pool.lock().unwrap();

            let chunk_render_manager = render_data.render_managers.get_mut("gpass").unwrap();
            chunk_render_manager
                .cull_and_draw_chunks(
                    &mesh_manager,
                    &mat_man,
                    &mut draw_cmd,
                    &mut ccmd,
                    descriptor_pool.borrow_mut(),
                    frame_index.index(),
                    &cam_data,
                )
                .unwrap();

            ccmd.end().unwrap();
            // draw_cmd.end().unwrap();

            gpass.submit_cmd(draw_cmd).unwrap();
            rp_man.submit_compute(ccmd);
        }
    }
}

pub fn register_render_data(game: &mut Game) -> eyre::Result<()> {
    let chunkrender_data = render_system::ChunkRendererData::new(
        &game.core, //
        &game.world.fetch::<RenderPassManager>(),
        &mut game.world.fetch_mut::<MaterialManager>(),
    )?;

    game.world.insert(chunkrender_data);
    game.world.insert(ChunkMeshManager::new(&game.core)?);

    game.insert_frame_task(Box::new(|w, d| {
        d.add(render_system::ChunkRenderer, "chunk render", &["chunk mesh"]);
    }));

    Ok(())
}
