use ash::vk;
use magma_renderer::engine::material::*;
use magma_renderer::{core::*, engine::mesh_manager::MeshManager};
use std::sync::Arc;

use specs::prelude::*;

use crate::game::{self, CameraData, Game};

use super::{
    renderpassmanager::{self, RenderPassManager},
    RenderAble,
};



pub fn create_cube_mesh(core: &Arc<Core>, cmd: &mut CommandBuffer) -> eyre::Result<magma_renderer::engine::mesh::Mesh> {
    let verticies = [
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [1.0, 0.0, 1.0],
        [0.0, 1.0, 1.0],
        [1.0, 1.0, 1.0],
    ];

    let indicies = [
        2, 6, 7, 2, 7, 3, //Top
        4, 0, 5, 5, 0, 1, //Bottom
        2, 0, 6, 0, 4, 6, //Left
        1, 3, 7, 1, 7, 5, //Right
        0, 2, 3, 0, 3, 1, //Front
        4, 7, 6, 4, 5, 7, //Back
    ];

    magma_renderer::engine::mesh::MeshBuilder::new().set_positions(&verticies).set_indicies(&indicies).build(core, cmd)

    // Ok(Mesh {
    //     verticies: core.create_buffer_from_slice(vk::BufferUsageFlags::VERTEX_BUFFER, &verticies)?,
    //     indicies: core.create_buffer_from_slice(vk::BufferUsageFlags::INDEX_BUFFER, &indicies)?,
    //     material_id,
    // })
}

pub struct CubePrefab(pub RenderAble);

pub fn init_cube(game: &mut Game) -> eyre::Result<()> {
    let cube_prefab = {
        let core = &game.core;

        let mut cmd = core.new_cmd();
        cmd.begin()?;

        let rp_man = game.world.fetch::<RenderPassManager>();
        let rp = rp_man.get_subpass("gpass").unwrap();

        let mut material_system = game.world.fetch_mut::<MaterialManager>();
        // material_system.set_vertex_layout("cube".into(), MeshVertex::get_desciption());

        let materialid = material_system.load_material(&mut cmd, "res/cube.mat.yaml".into())?;

        let mut mesh_man = game.world.fetch_mut::<MeshManager>();

        let meshid = mesh_man.register_mesh(create_cube_mesh(&game.core, &mut cmd)?);

        cmd.end()?;
        cmd.immediate_submit()?;

        CubePrefab(RenderAble { meshid, materialid })
    };

    game.world.insert(cube_prefab);

    // game.world.register::<Cube>();


    Ok(())
}
