use std::mem::swap;

use glam::*;
use specs::prelude::*;

use self::ray::Ray;

use super::{
    voxels::{ChunkRef, Tile, VoxelWorld, CHUNK_SIZE},
    DeltaTime, Game, Transform,
};

pub mod ray;

pub struct Collider {
    pub box_size: Vec3,
}

pub struct AABB {
    pub begin: Vec3,
    pub end: Vec3,
}

impl Collider {
    pub fn to_aabb(&self, tranform: &Transform) -> AABB { AABB { begin: tranform.pos, end: tranform.pos + self.box_size } }
}

impl AABB {
    fn aabb_1d(a_beg: f32, a_end: f32, b_beg: f32, b_end: f32) -> bool {
        (a_beg < b_beg && b_beg < a_end) || (b_beg < a_beg && a_beg < b_end)
    }

    fn aabb_1d_with_collision_point(a_beg: f32, a_end: f32, b_beg: f32, b_end: f32) -> Option<f32> {
        if a_beg < b_beg && b_beg < a_end {
            Some(a_end)
        } else if b_beg < a_beg && a_beg < b_end {
            Some(a_beg)
        } else {
            None
        }
    }

    pub fn check_collision_with_collision_point(&self, other: &Self) {}

    pub fn check_collision(&self, other: &Self) -> bool {
        Self::aabb_1d(self.begin.x, self.end.x, other.begin.x, other.end.x)
            && Self::aabb_1d(self.begin.y, self.end.y, other.begin.y, other.end.y)
            && Self::aabb_1d(self.begin.z, self.end.z, other.begin.z, other.end.z)
    }
}

impl Component for Collider {
    type Storage = DenseVecStorage<Self>;
}

#[derive(Default)]
pub struct Velocity {
    pub velocity: Vec3,
    pub old_velocity: Vec3,
    pub mass: f32,
    pub affected_by_gravity: bool,
}

impl Component for Velocity {
    type Storage = HashMapStorage<Self>;
}

pub struct AddForce {
    pub force: Vec3,
    pub duration: f32,
}

#[derive(Default)]
pub struct AddedForces {
    pub forces: Vec<AddForce>,
}

impl Component for AddedForces {
    type Storage = HashMapStorage<Self>;
}

impl AddedForces {
    pub fn add_force(&mut self, force: Vec3, duration: f32) { self.forces.push(AddForce { force, duration }); }
    pub fn calculate_added_velocity(&mut self, delta_time: f32, mass: f32) -> Vec3 {
        if self.forces.len() == 0 {
            return Vec3::ZERO;
        }
        let mut added_velocity = Vec3::ZERO;
        let inverse_mass = 1.0 / mass;
        for i in (self.forces.len() - 1)..=0 {
            let force = &mut self.forces[i];
            if force.duration > delta_time {
                force.duration -= delta_time;
                added_velocity += force.force * (inverse_mass * delta_time);
            } else {
                added_velocity += force.force * (inverse_mass * force.duration);
                self.forces.swap_remove(i);
            }
        }
        added_velocity
    }
}

struct ForceSystem;

impl<'a> System<'a> for ForceSystem {
    type SystemData = (WriteStorage<'a, Velocity>, WriteStorage<'a, AddedForces>, ReadExpect<'a, DeltaTime>);

    fn run(&mut self, (mut velocities, mut added_forces, delta_time): Self::SystemData) {
        const GRAVITY: f32 = 9.8;

        let delta_time = delta_time.0 as f32;

        for (vel, f) in (&mut velocities, (&mut added_forces).maybe()).join() {
            let mut new_vel = vel.velocity;
            new_vel.y += -GRAVITY * delta_time;
            if let Some(f) = f {
                new_vel += f.calculate_added_velocity(delta_time, vel.mass);
            }
            (vel.old_velocity, vel.velocity) = (vel.velocity, new_vel);
        }
    }
}

struct VelocitySystem;

impl<'a> System<'a> for VelocitySystem {
    type SystemData = (
        ReadExpect<'a, DeltaTime>,
        WriteStorage<'a, Velocity>,
        WriteStorage<'a, super::Transform>,
        ReadStorage<'a, Collider>,
        ReadExpect<'a, VoxelWorld>,
    );

    fn run(&mut self, (delta_time, mut velocities, mut transforms, colliders, voxel_world): Self::SystemData) {
        let delta_time = delta_time.0 as f32;

        for (vel, t, collider) in (&mut velocities, &mut transforms, colliders.maybe()).join() {
            // let mut avarage_vel = (vel.old_velocity + vel.velocity) * 0.5;
            let mut avarage_vel = vel.velocity;
            let move_vec = avarage_vel * delta_time;

            let Some(collider) = collider else {
                t.pos = t.pos + move_vec;
                continue
            };

            let mut aabb = collider.to_aabb(t);
            aabb.end += move_vec.max(Vec3::ZERO);
            aabb.begin += move_vec.min(Vec3::ZERO);

            let mut tile_colliders = Vec::<AABB>::new();

            //get near chunks tile boundry boxes
            let beg_chunk = (aabb.begin / CHUNK_SIZE as f32).floor().as_ivec3();
            let end_chunk = (aabb.end / CHUNK_SIZE as f32).floor().as_ivec3();

            for x in beg_chunk.x..=end_chunk.x {
                for y in beg_chunk.y..=end_chunk.y {
                    for z in beg_chunk.z..=end_chunk.z {
                        let Some(chunk) = voxel_world.get_chunk(&[x,y,z]) else {continue};
                        chunk.append_overlapping_aabb(&aabb, &mut tile_colliders);
                    }
                }
            }

            let half_size = collider.box_size * 0.5;
            let tmid_point = t.pos + half_size;
            let ray = Ray::new(tmid_point, move_vec);

            let mut ray_hits: Vec<_> = tile_colliders
                .iter()
                .enumerate()
                .filter_map(|(i, t_aabb)| {
                    let extended_aabb = AABB { begin: t_aabb.begin - half_size, end: t_aabb.end + half_size };
                    ray.test_aabb(&extended_aabb, true).map(|h| (h.t, i as u32))
                })
                .collect();

            ray_hits.sort_by(|ha, hb| ha.0.total_cmp(&hb.0));

            for (_, i) in ray_hits {
                let t_aabb = &tile_colliders[i as usize];
                let extended_aabb = AABB { begin: t_aabb.begin - half_size, end: t_aabb.end + half_size };
                let updated_ray = Ray::new(tmid_point, avarage_vel * delta_time);
                let Some(hit) = updated_ray.test_aabb(&extended_aabb, true) else{continue};
                avarage_vel += hit.cn * avarage_vel.abs() * (1.0 - hit.t);
            }

            t.pos += avarage_vel * delta_time;
            vel.velocity = avarage_vel;

            // vel.velocity = vel.old_velocity + (avarage_vel - vel.old_velocity) * 2.0;
        }
    }
}

impl<'a> ChunkRef<'a> {
    pub fn append_overlapping_aabb(&self, aabb: &AABB, append_colliders: &mut Vec<AABB>) {
        let chunk_world_pos = self.world_pos();

        let chunk_aabb = AABB {
            begin: chunk_world_pos,
            end: chunk_world_pos + Vec3::splat(CHUNK_SIZE as f32),
        };

        if chunk_aabb.check_collision(aabb) == false {
            return;
        }

        let relative_beg = (aabb.begin - chunk_world_pos).max(Vec3::ZERO);
        let relative_end = aabb.end - chunk_world_pos;

        for x in (relative_beg.x.floor() as usize)..=(relative_end.x.floor() as usize).min(CHUNK_SIZE - 1) {
            for y in (relative_beg.y.floor() as usize)..=(relative_end.y.floor() as usize).min(CHUNK_SIZE - 1) {
                for z in (relative_beg.z.floor() as usize)..=(relative_end.z.floor() as usize).min(CHUNK_SIZE - 1) {
                    let tile = self.get_block(x, y, z);
                    tile.append_colliders(chunk_world_pos + vec3(x as f32, y as f32, z as f32), append_colliders);
                }
            }
        }
    }
}

impl Tile {
    pub fn append_colliders(&self, pos: Vec3, vec: &mut Vec<AABB>) {
        if self.properties().is_transparent {
            return;
        }

        vec.push(AABB { begin: pos, end: pos + vec3(1.0, 1.0, 1.0) })
    }
}

pub fn init(game: &mut Game) {
    game.world.register::<Velocity>();
    game.world.register::<Collider>();
    game.world.register::<AddedForces>();

    game.insert_frame_task(Box::new(|w, d| {
        d.add(ForceSystem, "forces", &[]);
        d.add(VelocitySystem, "velocities", &["forces"]);
    }));
}
