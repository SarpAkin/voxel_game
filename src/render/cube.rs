use ash::vk;
use nalgebra::{Isometry3, Matrix4, Rotation3, Translation, Vector3};
use std::sync::Arc;
use magma_renderer::core::*;

use specs::prelude::*;

use crate::game::{self, Game, CameraData};

use super::{create_pipeline, Mesh, MeshVertex, renderpassmanager::{self, RenderPassManager}};

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
    type SystemData =
        (ReadStorage<'a, crate::game::Transform>,ReadStorage<'a,Cube>, WriteExpect<'a, renderpassmanager::RenderPassManager>,ReadExpect<'a,CameraData>);

    #[allow(unused_parens)]
    fn run(&mut self, (transform,cube, rpman,cam): Self::SystemData) {
        let gpass = rpman.get_subpass("gpass").unwrap();
        let mut cmd = gpass.new_cmd().unwrap();
        let proj_view = cam.proj_view;

        
        for (transform,_) in (&transform,&cube).join() {
            // println!("{}",transform.pos);
            let mvp = proj_view * transform.matrix();
            // println!("{}",mvp);

            self.cube_mesh.draw(&mut cmd, &mvp);
        }

        gpass.submit_cmd(cmd).unwrap();
    }
}

pub fn create_cube_mesh(core: &Arc<Core>, renderpass: &dyn Renderpass) -> eyre::Result<Mesh> {
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
        2, 6, 7, 2, 3, 7, //Top
        0, 4, 5, 0, 1, 5, //Bottom
        0, 2, 6, 0, 4, 6, //Left
        1, 3, 7, 1, 5, 7, //Right
        0, 2, 3, 0, 1, 3, //Front
        4, 6, 7, 4, 5, 7, //Back
    ];

    Ok(Mesh {
        verticies: core.create_buffer_from_slice(vk::BufferUsageFlags::VERTEX_BUFFER, &verticies)?,
        indicies: core.create_buffer_from_slice(vk::BufferUsageFlags::INDEX_BUFFER, &indicies)?,
        metarial: create_pipeline(core, renderpass)?,
    })
}

pub fn init_cube(game: &mut Game) -> eyre::Result<()> {
    let mesh = Arc::new(create_cube_mesh(&game.core, game.world.fetch::<RenderPassManager>().get_subpass("gpass").unwrap().renderpass())?);
    game.world.register::<Cube>();

    game.insert_frame_task(Box::new(move |world, d| {
        //
        d.add(RenderSys { cube_mesh: mesh.clone() }, "render cubes", &[]);
    }));

    Ok(())
}
