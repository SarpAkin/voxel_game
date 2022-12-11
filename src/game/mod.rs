use std::{
    f32::consts::PI,
    sync::{
        mpsc::{sync_channel, SyncSender},
        Arc, Mutex,
    },
};

use ash::vk;
use eyre::Result;
use magma_renderer::{core::*, window::*};
use nalgebra::{Isometry3, Matrix4, Point3, Rotation3, Vector3};
use specs::prelude::*;
use std::sync::mpsc::channel;

pub mod voxels;
pub mod physics;

use crate::{game::physics::{Velocity, Collider}, render::Cube};

use self::voxels::VoxelWorld;

use super::render;


pub struct DeltaTime(pub f64);

pub struct Transform {
    pub pos: Point3<f32>,
    pub yaw: f32,
    pub pitch: f32,
}

impl Component for Transform {
    type Storage = VecStorage<Self>;
}

fn _eular_from_vector(vec: Vector3<f32>) -> (f32, f32) {
    let yaw = f32::atan2(vec.z, vec.x);
    let pitch = f32::asin(vec.y / vec.magnitude());
    (yaw, pitch)
}

impl Transform {
    pub fn new(x: f32, y: f32, z: f32) -> Transform { Self { pos: Point3::new(x, y, z), yaw: 0.0, pitch: 0.0 } }

    pub fn direction(&self) -> Vector3<f32> {
        let pcos = self.pitch.cos();

        let x = self.yaw.cos() * pcos;
        let z = self.yaw.sin() * pcos;
        let y = self.pitch.sin();

        Vector3::new(x, y, z)
    }

    pub fn matrix(
        &self,
    ) -> nalgebra::Matrix<f32, nalgebra::Const<4>, nalgebra::Const<4>, nalgebra::ArrayStorage<f32, 4, 4>> {
        // let iso = Isometry3::new(self.pos.coords, Vector3::zeros());
        // let a = Rotation3::from_euler_angles(0.0, self.pitch, self.yaw);

        let mut m = Matrix4::from_euler_angles(0.0, self.pitch, self.yaw);
        m[(0, 3)] += self.pos.x;
        m[(1, 3)] += self.pos.y;
        m[(2, 3)] += self.pos.z;
        m
    }
}

pub struct Camera {
    pub fovy: f32,
    pub znear: f32,
    pub zfar: f32,
}

impl Camera {
    #[rustfmt::skip]
    pub fn proj(&self, (width, height): (u32, u32)) -> Matrix4<f32> {
        //https://vincent-p.github.io/posts/vulkan_perspective_matrix/

        let aspect_ratio = width as f32 / height as f32;
        let fov_rad = self.fovy.to_radians();
        let focal_length = 1.0 / f32::tan(fov_rad / 2.0);

        let x = focal_length / aspect_ratio;
        let y = -focal_length;
        let a = self.znear / (self.zfar - self.znear);
        let b = self.zfar * a;

        Matrix4::new(
            x,  0.0, 0.0,   0.0, //
            0.0,y,   0.0,   0.0, //
            0.0,0.0,-1.0 - a,-b, //
            0.0,0.0,-1.0,   0.0, //
        )
    }
}

pub type FrameTask = Box<dyn Fn(&mut World, &mut DispatcherBuilder)>;

pub struct Game {
    pub world: World,
    pub core: Arc<Core>,
    camera: Camera,
    player: Transform,
    frame_tasks: Vec<FrameTask>,
    pub descriptor_pool: DescriptorPool,
    // gpass: DeferedPass,
}

static UP: Vector3<f32> = Vector3::new(0.0, 1.0, 0.0);

pub struct CameraData {
    pub proj_view: Matrix4<f32>,
}

pub struct FrameData {
    pub descriptor_pool: Mutex<DescriptorPool>,
}

pub struct RenderGlobals {
    core: Arc<Core>,
    frame_datas: Box<[FrameData]>,
    frame_index: u32,
}

impl RenderGlobals {
    pub fn frame_data(&self) -> &FrameData { &self.frame_datas[self.frame_index as usize] }
    pub fn core(&self) -> &Arc<Core> { &self.core }
    fn next_frame(&mut self) { self.frame_index = (self.frame_index + 1) % 2; }
    fn start_frame(&mut self) -> Result<(), vk::Result> { self.frame_data().descriptor_pool.lock().unwrap().reset() }
}

impl Game {
    pub fn insert_frame_task(&mut self, task: FrameTask) { self.frame_tasks.push(task); }

    pub fn new(core: &Arc<Core>, renderpass: &dyn Renderpass) -> Box<Self> {
        let mut world = World::new();
        world.register::<Transform>();
        world.insert(core.clone());

        let camera = Camera { fovy: 90.0, znear: 0.1, zfar: 200.0 };

        let mut game = Box::new(Game {
            world,
            camera,
            player: Transform { pos: Point3::origin(), yaw: 0.0, pitch: 0.0 },
            frame_tasks: vec![],
            core: core.clone(),
            descriptor_pool: DescriptorPool::new(core),
        });

        game.world.insert(RenderGlobals {
            core: core.clone(),
            frame_datas: (0..2).map(|_| FrameData { descriptor_pool: Mutex::new(DescriptorPool::new(&core)) }).collect(),
            frame_index: 0,
        });

        render::renderpasses::init(&mut game, renderpass);

        voxels::init(&mut game);
        render::init_cube(&mut game).unwrap();
        render::chunk_render::init(&mut game, renderpass);

        physics::init(&mut game);

        game
    }

    pub fn tick(&mut self, delta_time: f64, cmd: &mut CommandBuffer, ar: &mut Window) -> Result<()> {
        self.world.insert(DeltaTime(delta_time));
        handle_player_movement(&mut self.world,&mut self.player, delta_time, ar);

        render::renderpasses::prepare_render(self, &ar.renderpass).unwrap();


        let proj_view = self.camera.proj(ar.renderpass.extends())
            * Isometry3::look_at_rh(&self.player.pos, &(self.player.pos + self.player.direction()), &UP).to_homogeneous();

        self.world.insert(CameraData { proj_view });

        self.world.write_resource::<RenderGlobals>().start_frame()?;

        //build the dispatcher
        let mut dbuilder = DispatcherBuilder::new();

        self.frame_tasks.iter().for_each(|t| (*t)(&mut self.world, &mut dbuilder));

        let mut dispatcher = dbuilder.build();

        dispatcher.dispatch_seq(&self.world);
        dispatcher.dispatch_thread_local(&self.world);

        //execute rendering commands
        render::renderpasses::render(self, cmd, &ar.renderpass).unwrap();

        self.world.write_resource::<RenderGlobals>().next_frame();

        Ok(())
    }
}

fn handle_player_movement(world:&mut World,player_transform: &mut Transform, delta_time: f64, ar: &mut Window) {
    // use winit::event::MouseButton;
    if ar.get_key(Key::Escape) == InputState::Pressed {
        ar.unlock_cursor();
    }

    if ar.get_mouse_button(MouseButton::Button1) == InputState::Pressed {
        ar.lock_cursor();
    }

    let speed = 5.6;

    let sensivity = 0.005;
    let (mx, my) = ar.get_mouse_movement();
    let max_vertical_rotation = f32::to_radians(89.0);

    player_transform.yaw += mx * sensivity;
    player_transform.pitch =
        f32::clamp(player_transform.pitch + -my * sensivity, -max_vertical_rotation, max_vertical_rotation);

    fn key_to_scaler(state: InputState) -> f32 {
        match state {
            InputState::Pressed => 1.0,
            InputState::Released => 0.0,
        }
    }

    let mut move_vector = Vector3::<f32>::zeros();

    move_vector.x += key_to_scaler(ar.get_key(Key::A));
    move_vector.x -= key_to_scaler(ar.get_key(Key::D));
    move_vector.z += key_to_scaler(ar.get_key(Key::W));
    move_vector.z -= key_to_scaler(ar.get_key(Key::S));
    move_vector.y += key_to_scaler(ar.get_key(Key::Space));
    move_vector.y -= key_to_scaler(ar.get_key(Key::LeftControl));

    let forward = Vector3::new(player_transform.yaw.cos(), 0.0f32, player_transform.yaw.sin());
    let right = Vector3::new(forward.z, 0.0, -forward.x); //90 degrees rotated clockwise

    let final_vec = forward * move_vector.z + right * move_vector.x + Vector3::new(0.0, move_vector.y, 0.0);

    player_transform.pos += final_vec * delta_time as f32 * speed;


    struct TimeSincelastBox(f32);

    

    if ar.get_key(Key::G) == InputState::Pressed{
        if let Some(time) = world.get_mut::<TimeSincelastBox>() {
            if time.0 < 1.0{
                time.0 +=   delta_time as f32;
                return;
            }
            else{
                time.0 = 0.0;
            }
        }
        else {
            world.insert(TimeSincelastBox(0.0));
        }

        world.create_entity()
            .with(Transform::new(player_transform.pos.x, player_transform.pos.y, player_transform.pos.z))
            .with(Velocity::default())
            .with(Collider{box_size:Vector3::new(1.0,2.0,1.0)})
            .with(Cube)
            .build();
    }

    // if final_vec != Vector3::zeros() {
    //     println!("move direction {}, final pos {:?}",final_vec,player_transform.pos)
    // }
}
