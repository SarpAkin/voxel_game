use std::sync::Arc;

use ash::vk;
use bytemuck::{Pod, Zeroable};
use magma_renderer::core::*;
use nalgebra::{Matrix4, Vector3};
use specs::prelude::*;

use crate::{
    game::{self, voxels::ChunkComponent, Game},
    include_glsl,
    render::{self, renderpassmanager::RenderPassManager},
};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Push {
    mvp: [[f32; 4]; 4],
}

pub struct ChunkMesh {
    pub(crate) verticies: Buffer<super::ChunkVertex>,
}

impl ChunkMesh {}

impl Component for ChunkMesh {
    type Storage = DenseVecStorage<Self>;
}

pub struct RenderData {
    pipeline: Arc<Pipeline>,
    indicies: Buffer<u16>,
    texture_0: Image,
    set: vk::DescriptorSet,
    sampler: Sampler, //albedao spec
}

impl RenderData {
    fn prep_draw(&self, cmd: &mut CommandBuffer) {
        cmd.bind_pipeline(&self.pipeline);
        cmd.bind_index_buffer(self.indicies.as_slice());
        cmd.bind_descriptor_set(0, self.set);
    }

    fn draw_chunk(&self, cmd: &mut CommandBuffer, mesh: &ChunkMesh, mvp: &Matrix4<f32>) {
        cmd.push_constant(&Push { mvp: mvp.data.0 }, vk::ShaderStageFlags::VERTEX, 0);
        cmd.bind_vertex_buffers(&[&mesh.verticies]);
        unsafe {
            cmd.device().cmd_draw_indexed(cmd.inner(), mesh.verticies.size() / 4 * 6, 1, 0, 0, 0);
        }
    }
}

pub struct Renderer {}

impl<'a> System<'a> for Renderer {
    type SystemData = (
        ReadExpect<'a, RenderData>,
        ReadExpect<'a, render::renderpassmanager::RenderPassManager>,
        ReadExpect<'a, game::RenderGlobals>,
        ReadExpect<'a, game::CameraData>,
        ReadStorage<'a, ChunkMesh>,
        ReadStorage<'a, ChunkComponent>,
    );
    //Render Chunks
    fn run(&mut self, (render_data, rpman, _globals, cam, meshes, chunks): Self::SystemData) {
        let gpass = rpman.get_subpass("gpass").unwrap();
        let mut cmd = gpass.new_cmd().unwrap();

        render_data.prep_draw(&mut cmd);

        for (c, m) in (&chunks, &meshes).join() {
            let [x, y, z] = c.chunkpos;

            let mvp = cam.proj_view * Matrix4::new_translation(&(Vector3::new(x as f32, y as f32, z as f32) * 32.0));

            render_data.draw_chunk(&mut cmd, m, &mvp);
        }

        gpass.submit_cmd(cmd).unwrap();
    }
}

pub fn create_pipeline(
    core: &Arc<Core>,
    renderpass: &dyn Renderpass,
    set_layout: vk::DescriptorSetLayout,
) -> eyre::Result<Arc<Pipeline>> {
    let vert_code = ShaderModule::new(core, include_glsl!("res/chunk.vert"))?;
    let frag_code = ShaderModule::new(core, include_glsl!("res/chunk.frag"))?;

    let playout =
        PipelineLayoutBuilder::new().add_set(set_layout).add_push::<Push>(vk::ShaderStageFlags::VERTEX, 0).build(core)?;

    let pipeline = GPipelineBuilder::new()
        .set_pipeline_layout(playout)
        .add_shader_stage(vk::ShaderStageFlags::VERTEX, &vert_code.module())
        .add_shader_stage(vk::ShaderStageFlags::FRAGMENT, &frag_code.module())
        .set_rasterization(vk::PolygonMode::FILL, vk::CullModeFlags::BACK)
        .set_depth_testing(true)
        .set_topology(vk::PrimitiveTopology::TRIANGLE_LIST)
        .set_vertex_description(super::ChunkVertex::get_desciption())
        .build(core, renderpass, 0)?;

    Ok(pipeline)
}

pub fn register_render_data(game: &mut Game, renderpass: &dyn Renderpass) -> eyre::Result<()> {
    let core = &game.core;

    let set_layout = DescriptorSetLayoutBuilder::new() //
        .add_sampler(vk::ShaderStageFlags::FRAGMENT, 1)
        .build(core)?;

    let mut cmd = game.core.new_cmd();
    cmd.begin()?;

    let rpman = game.world.fetch_mut::<RenderPassManager>();

    let pipeline = create_pipeline(core, rpman.get_subpass("gpass").unwrap().renderpass(), set_layout)?;
    drop(rpman);

    let texture = cmd.load_image_from_file("res/voxel_tilemap.png", vk::ImageUsageFlags::SAMPLED)?;
    let indicies = cmd.gpu_buffer_from_data(
        &(0..16384).flat_map(|i| [i * 4 + 0, i * 4 + 1, i * 4 + 2, i * 4 + 2, i * 4 + 1, i * 4 + 3]).collect::<Vec<u16>>(),
        vk::BufferUsageFlags::INDEX_BUFFER,
    )?;

    let sampler = core.create_sampler(vk::Filter::NEAREST, None);

    let set = DescriptorSetBuilder::new() //
        .add_sampled_image(&texture, *sampler)
        .build(set_layout, &mut game.descriptor_pool)?;

    game.world.insert(RenderData { pipeline, indicies, texture_0: texture, set, sampler });

    cmd.end()?;
    cmd.immediate_submit()?;

    Ok(())
}
