use ash::vk;
use magma_renderer::core::*;
use magma_renderer::engine::material::*;
use std::sync::Arc;

use specs::prelude::*;

use crate::game::{self, CameraData, Game};

use super::{
    create_pipeline,
    renderpassmanager::{self, RenderPassManager},
    Mesh, MeshVertex,
};

#[derive(Default)]
pub struct Cube;
impl Component for Cube {
    type Storage = NullStorage<Self>;
}

pub struct RenderSys {
    // pub cmd: &'a mut vk_ash_engine::core::CommandBuffer,
    // pub proj_view: &'a Matrix4<f32>,
    pub cube_mesh: Arc<Mesh>,
}

impl<'a> System<'a> for RenderSys {
    #[allow(unused_parens)]
    type SystemData = (
        ReadStorage<'a, crate::game::Transform>,
        ReadStorage<'a, Cube>,
        WriteExpect<'a, renderpassmanager::RenderPassManager>,
        ReadExpect<'a, CameraData>,
        ReadExpect<'a, MaterialManager>,
    );

    #[allow(unused_parens)]
    fn run(&mut self, (transform, cube, rpman, cam, mat_man): Self::SystemData) {
        let gpass = rpman.get_subpass("gpass").unwrap();
        let mut cmd = gpass.new_cmd().unwrap();
        let proj_view = cam.proj_view;

        for (transform, _) in (&transform, &cube).join() {
            // println!("{}",transform.pos);
            let mvp = proj_view * transform.matrix();
            // println!("{}",mvp);

            self.cube_mesh.draw(&mut cmd, &mvp, &mat_man);
        }

        gpass.submit_cmd(cmd).unwrap();
    }
}

pub fn create_cube_mesh(core: &Arc<Core>, material_id: MaterialID) -> eyre::Result<Mesh> {
    let verticies = [
        MeshVertex { pos: [0.0, 0.0, 0.0] },
        MeshVertex { pos: [1.0, 0.0, 0.0] },
        MeshVertex { pos: [0.0, 1.0, 0.0] },
        MeshVertex { pos: [1.0, 1.0, 0.0] },
        MeshVertex { pos: [0.0, 0.0, 1.0] },
        MeshVertex { pos: [1.0, 0.0, 1.0] },
        MeshVertex { pos: [0.0, 1.0, 1.0] },
        MeshVertex { pos: [1.0, 1.0, 1.0] },
    ];

    let indicies = [
        2, 6, 7, 2, 7, 3, //Top
        4, 0, 5, 5, 0, 1, //Bottom
        2, 0, 6, 0, 4, 6, //Left
        1, 3, 7, 1, 7, 5, //Right
        0, 2, 3, 0, 3, 1, //Front
        4, 7, 6, 4, 5, 7, //Back
    ];

    Ok(Mesh {
        verticies: core.create_buffer_from_slice(vk::BufferUsageFlags::VERTEX_BUFFER, &verticies)?,
        indicies: core.create_buffer_from_slice(vk::BufferUsageFlags::INDEX_BUFFER, &indicies)?,
        material_id,
    })
}

pub fn init_cube(game: &mut Game) -> eyre::Result<()> {
    let mesh = {
        let core = &game.core;

        let mut cmd = core.new_cmd();
        cmd.begin()?;

        let rp_man = game.world.fetch::<RenderPassManager>();
        let rp = rp_man.get_subpass("gpass").unwrap();

        let mut material_system = game.world.fetch_mut::<MaterialManager>();
        material_system.set_vertex_layout("cube".into(), MeshVertex::get_desciption());

        let mesh = Arc::new(create_cube_mesh(
            &game.core,
            material_system.load_material(&mut cmd, "res/cube.mat.yaml".into())?,
        )?);

        cmd.end()?;
        cmd.immediate_submit()?;

        mesh
    };

    game.world.register::<Cube>();

    game.insert_frame_task(Box::new(move |_world, d| {
        //
        d.add(RenderSys { cube_mesh: mesh.clone() }, "render cubes", &[]);
    }));

    Ok(())
}
