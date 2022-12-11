use super::AABB;
use nalgebra::{Point3, Vector3};
use std::mem::swap;

pub struct Ray {
    pos: Point3<f32>,
    dir: Vector3<f32>,
    inv_dir: Vector3<f32>,
}

pub struct RayHit<'a> {
    pub ray: &'a Ray,
    pub t: f32,
    pub cn: Vector3<f32>,
}

impl Ray {
    pub fn new(pos: Point3<f32>, dir: Vector3<f32>) -> Ray { Ray { pos, dir, inv_dir: dir.map(|n| 1.0 / n) } }
    pub fn pos(&self) -> Point3<f32> { self.pos }
    pub fn dir(&self) -> Vector3<f32> { self.dir }

    pub fn test_aabb(&self, aabb: &AABB, in_range: bool) -> Option<RayHit> {
        //calculate near and far values
        let mut tn = (aabb.begin - self.pos).component_mul(&self.inv_dir);
        let mut tf = (aabb.end - self.pos).component_mul(&self.inv_dir);

        //check for the case of 0 / 0
        if tn.x.is_nan() || tn.y.is_nan() || tn.z.is_nan() || tf.x.is_nan() || tf.y.is_nan() || tf.z.is_nan() {
            return None;
        }

        //sort near and far values simce direction of ray can be negative
        for i in 0..3 {
            if tn[i] > tf[i] {
                swap(&mut tn[i], &mut tf[i])
            };
        }

        // nears has to be smaller than fars in case of collisions
        let t_hit = tn.max();
        if t_hit > tf.min() {
            return None;
        }

        // we do not want collision in negative direction
        if t_hit < 0.0 {
            return None;
        }

        // return none if we want only in range hits
        if in_range && t_hit > 1.0 {
            return None;
        }

        let cn = if tn.x > tn.y {
            if tn.x > tn.z {
                //biggest is x
                if self.dir.x > 0.0 { Vector3::new(-1.0, 0.0, 0.0) } else { Vector3::new(1.0, 0.0, 0.0) }
            } else {
                //biggest is z
                if self.dir.z > 0.0 { Vector3::new(0.0, 0.0, -1.0) } else { Vector3::new(0.0, 0.0, 1.0) }
            }
        } else {
            if tn.y > tn.z {
                //biggest is y
                if self.dir.y > 0.0 { Vector3::new(0.0, -1.0, 0.0) } else { Vector3::new(0.0, 1.0, 0.0) }
            } else {
                //biggest is z
                if self.dir.z > 0.0 { Vector3::new(0.0, 0.0, -1.0) } else { Vector3::new(0.0, 0.0, 1.0) }
            }
        };

        Some(RayHit { ray: &self, t: t_hit, cn })
    }
}

impl<'a> RayHit<'a> {
    pub fn hit_point(&self) -> Point3<f32> { self.ray.pos + self.ray.dir * self.t }
    pub fn is_in_range(&self) -> bool { self.t <= 1.0 }
}
