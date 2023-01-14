use std::sync::Arc;

use ash::vk;
use bytemuck::{Pod, Zeroable};
use glam::*;
use magma_renderer::auto_description;
use magma_renderer::core::*;

pub mod chunk_render;
mod cube;
pub mod renderpassmanager;
pub use cube::*;
use magma_renderer::engine::material::MaterialID;
use magma_renderer::engine::material::MaterialManager;

use crate::game::Game;
pub mod renderpasses;

#[cfg(debug_assertions)]
#[macro_export]
macro_rules! include_glsl {
    ($p:tt) => {
        vk_shader_macros::include_glsl!($p, debug)
    };
}

#[cfg(not(debug_assertions))]
#[macro_export]
macro_rules! include_glsl {
    ($p:tt) => {
        vk_shader_macros::include_glsl!($p, strip)
    };
}

auto_description!(
    #[repr(C)]
    #[derive(Clone, Copy, Zeroable, Pod)]
    struct MeshVertex {
        pos: [f32; 3],
    }
);

pub struct Mesh {
    verticies: Buffer<MeshVertex>,
    indicies: Buffer<u16>,
    material_id: MaterialID,
}

#[repr(C)]
#[derive(Pod, Clone, Copy, Zeroable)]
struct Push {
    mvp: [[f32; 4]; 4],
}

fn create_pipeline(core: &Arc<Core>, renderpass: &dyn Renderpass) -> eyre::Result<Arc<Pipeline>> {
    let vert_code = ShaderModule::new(core, include_glsl!("res/cube.vert"))?;
    let frag_code = ShaderModule::new(core, include_glsl!("res/cube.frag"))?;

    let playout = PipelineLayoutBuilder::new().add_push::<Push>(vk::ShaderStageFlags::VERTEX, 0).build(core)?;
    let pipeline = GPipelineBuilder::new()
        .set_pipeline_layout(playout)
        .add_shader_stage(vk::ShaderStageFlags::VERTEX, &vert_code.module())
        .add_shader_stage(vk::ShaderStageFlags::FRAGMENT, &frag_code.module())
        .set_rasterization(vk::PolygonMode::FILL, vk::CullModeFlags::NONE)
        .set_depth_testing(true)
        .set_topology(vk::PrimitiveTopology::TRIANGLE_LIST)
        .set_vertex_description(MeshVertex::get_desciption())
        .set_render_target(renderpass.get_subpasses()[0].get_render_target())
        .build(core)?;

    Ok(pipeline)
}

impl Mesh {
    fn draw(&self, cmd: &mut CommandBuffer, mvp: &Mat4, material_manager: &MaterialManager) {
        cmd.bind_material(&material_manager.get_material(self.material_id).unwrap());

        cmd.push_constant(&Push { mvp: mvp.to_cols_array_2d() }, vk::ShaderStageFlags::VERTEX, 0);
        // let v: Box<_> = self
        //     .verticies
        //     .get_data()
        //     .unwrap()
        //     .iter()
        //     .map(|v| mvp.transform_point(&Point3::from(v.pos)))
        //     .collect();
        // println!("{:?}", v);

        unsafe {
            let d = cmd.device();
            let c = cmd.inner();
            d.cmd_bind_vertex_buffers(c, 0, &[self.verticies.raw_buffer()], &[0]);
            d.cmd_bind_index_buffer(c, self.indicies.raw_buffer(), 0, vk::IndexType::UINT16);
            d.cmd_draw_indexed(c, self.indicies.size(), 1, 0, 0, 0);
            // d.cmd_draw(c, 6, 1, 0, 0);
        }
    }
}

#[repr(C)]
#[derive(Pod, Clone, Copy, Zeroable)]
pub struct VkDrawIndexedIndirectCommand {
    pub index_count: u32,
    pub instance_count: u32,
    pub first_index: u32,
    pub vertex_offset: i32,
    pub first_instance: u32,
}

unsafe fn foo(core: &Arc<Core>) {

    // core.device().cmd_draw_indexed_indirect_count(command_buffer, buffer, offset, count_buffer, count_buffer_offset, max_draw_count, stride)
}

pub fn init_material_system(game: &mut Game) {
    use magma_renderer::engine::material::*;

    let core = &game.core;
    let material_system = MaterialManager::new(core).unwrap();

    game.world.insert(material_system);
}
