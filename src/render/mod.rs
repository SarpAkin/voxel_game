pub mod chunk_render;
mod cube;
pub mod renderpasses;
pub mod renderpassmanager;

pub use cube::*;
use magma_renderer::engine::mesh_manager::MeshManager;
use specs::Join;
use specs::ReadExpect;
use specs::ReadStorage;
use specs::WorldExt;

use std::sync::Arc;

use ash::vk;
use bytemuck::{Pod, Zeroable};
use glam::*;

use magma_renderer::auto_description;
use magma_renderer::core::*;
use magma_renderer::engine::material::MaterialID;
use magma_renderer::engine::material::MaterialManager;
use magma_renderer::engine::mesh_manager::MeshID;

use specs::Component;
use specs::System;
use specs::VecStorage;

use crate::game;
use crate::game::CameraData;
use crate::game::Game;
use crate::game::Transform;

use self::renderpassmanager::RenderPassManager;

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

#[repr(C)]
#[derive(Pod, Clone, Copy, Zeroable)]
struct Push {
    mvp: [[f32; 4]; 4],
}

#[derive(Clone, Copy)]
pub struct RenderAble {
    pub meshid: MeshID,
    pub materialid: MaterialID,
}

impl Component for RenderAble {
    type Storage = VecStorage<Self>;
}

pub struct Renderer {}

impl<'a> System<'a> for Renderer {
    type SystemData = (
        ReadStorage<'a, Transform>,
        ReadStorage<'a, RenderAble>,
        //
        ReadExpect<'a, CameraData>,
        ReadExpect<'a, game::RenderGlobals>,
        ReadExpect<'a, RenderPassManager>,
        ReadExpect<'a, MaterialManager>,
        ReadExpect<'a, MeshManager>,
    );

    fn run(&mut self, (transforms, renderdatas, cam_data, render_globals, rp_man, mat_man, mesh_man): Self::SystemData) {
        let mut renderer = magma_renderer::engine::renderer::BatchRenderer::new();
        for (transform, rdata) in (&transforms, &renderdatas).join() {
            renderer.add_entity(transform.matrix(), rdata.meshid, rdata.materialid);
        }

        let gpass = rp_man.get_subpass("gpass").unwrap();
        let mut cmd = gpass.new_cmd().unwrap();

        renderer.flush_and_draw(&mut cmd, &cam_data.get_meshpass(), &mat_man, &mesh_man);

        gpass.submit_cmd(cmd).unwrap();
    }
}

// fn create_pipeline(core: &Arc<Core>, renderpass: &dyn Renderpass) -> eyre::Result<Arc<Pipeline>> {
//     let vert_code = ShaderModule::new(core, include_glsl!("res/cube.vert"))?;
//     let frag_code = ShaderModule::new(core, include_glsl!("res/cube.frag"))?;

//     let playout = PipelineLayoutBuilder::new().add_push::<Push>(vk::ShaderStageFlags::VERTEX, 0).build(core)?;
//     let pipeline = GPipelineBuilder::new()
//         .set_pipeline_layout(playout)
//         .add_shader_stage(vk::ShaderStageFlags::VERTEX, &vert_code.module())
//         .add_shader_stage(vk::ShaderStageFlags::FRAGMENT, &frag_code.module())
//         .set_rasterization(vk::PolygonMode::FILL, vk::CullModeFlags::NONE)
//         .set_depth_testing(true)
//         .set_topology(vk::PrimitiveTopology::TRIANGLE_LIST)
//         .set_vertex_description(MeshVertex::get_desciption())
//         .set_render_target(renderpass.get_subpasses()[0].get_render_target())
//         .build(core)?;

//     Ok(pipeline)
// }

// impl Mesh {
//     fn draw(&self, cmd: &mut CommandBuffer, mvp: &Mat4, material_manager: &MaterialManager) {
//         cmd.bind_material(&material_manager.get_material(self.material_id).unwrap());

//         cmd.push_constant(&Push { mvp: mvp.to_cols_array_2d() }, vk::ShaderStageFlags::VERTEX, 0);
//         // let v: Box<_> = self
//         //     .verticies
//         //     .get_data()
//         //     .unwrap()
//         //     .iter()
//         //     .map(|v| mvp.transform_point(&Point3::from(v.pos)))
//         //     .collect();
//         // println!("{:?}", v);

//         unsafe {
//             let d = cmd.device();
//             let c = cmd.inner();
//             d.cmd_bind_vertex_buffers(c, 0, &[self.verticies.raw_buffer()], &[0]);
//             d.cmd_bind_index_buffer(c, self.indicies.raw_buffer(), 0, vk::IndexType::UINT16);
//             d.cmd_draw_indexed(c, self.indicies.size(), 1, 0, 0, 0);
//             // d.cmd_draw(c, 6, 1, 0, 0);
//         }
//     }
// }

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

    let mesh_manager = MeshManager::new();
    game.world.insert(mesh_manager);

    game.world.insert(material_system);

    game.world.register::<RenderAble>();

    game.insert_frame_task(Box::new(move |_world, d| {
        d.add(Renderer {}, "render_meshes", &[]);
        //
        // d.add(RenderSys { cube_cube: mesh.clone() }, "render cubes", &[]);
    }));
}
