use std::sync::{mpsc::channel, Arc};

use noise::NoiseFn;

use super::*;

type ChunkModifier = Box<dyn Fn(&mut [Tile; CHUNK_VOLUME], &[i32; 3]) + Sync + Send>;

pub struct GeneratedChunk {
    pub voxels: Box<[Tile; CHUNK_VOLUME]>,
    pub pos: [i32; 3],
}

struct Config {
    seed: u64,
    modifiers: Vec<ChunkModifier>,
}

impl Config {
    fn generate(&self, pos: [i32; 3]) -> GeneratedChunk {
        let mut chunk = crate::util::boxed_slice_to_array((0..CHUNK_VOLUME).map(|_| AIR).collect()).unwrap();

        self.modifiers.iter().for_each(|m| m(&mut chunk, &pos));

        GeneratedChunk { voxels: chunk, pos }
    }
}

pub struct WorldGen {
    config: Arc<Config>,
    chunk_recv: std::sync::mpsc::Receiver<GeneratedChunk>,
    chunk_send: std::sync::mpsc::Sender<GeneratedChunk>,
}

impl WorldGen {
    pub fn new(seed: u64) -> WorldGen {
        let (chunk_send, chunk_recv) = channel::<GeneratedChunk>();
        // let (chunk_pos_send, chunk_pos_recv) = channel::<[i32; 3]>();

        let mut config = Config { seed, modifiers: vec![] };

        // noise::

        let n0 = CustomNoise::new(noise::OpenSimplex::new(seed as u32), 6.6, 0.065);
        let bn = CustomNoise::new(noise::OpenSimplex::new(seed as u32), 1.0, 0.0046);

        config.modifiers.push(Box::new(move |c, pos| {
            let desert_threshold = 0.4;
            let tundra_threshold = -0.35;

            let cx = (pos[0] as f64) * CHUNK_SIZE as f64;
            let cy = (pos[1] as f64) * CHUNK_SIZE as f64;
            let cz = (pos[2] as f64) * CHUNK_SIZE as f64;

            for iz in 0..CHUNK_SIZE {
                for ix in 0..CHUNK_SIZE {
                    let tz = cz + iz as f64;
                    let tx = cx + ix as f64;

                    let b = bn.get([tx, tz]);

                    let b0 = (1.0 - b) * 0.7;
                    let b02 = b0 * b0 * 1.3;

                    let base_height = b02 * 23.5 + 11.1;
                    let height_mul = b02 * 3.2;

                    let (under_ground, surface) = match b {
                        b if b > desert_threshold => (SAND, SAND),
                        b if b < tundra_threshold => (DIRT, SNOW),
                        _ => (DIRT, GRASS),
                    };

                    let height = height_mul * n0.get([tz, tx]) + base_height;
                    let stone_height = height - 4.0;
                    let dirt_height = height - 1.0;
                    for iy in 0..CHUNK_SIZE {
                        let ty = cy + iy as f64;

                        c[ix + iz * CHUNK_SIZE + iy * CHUNK_AREA] = match ty {
                            h if h < stone_height => STONE,
                            h if h < dirt_height => under_ground,
                            h if h < height => surface,
                            _ => break,
                        }
                    }
                }
            }
        }));

        let gen = WorldGen { config: Arc::new(config), chunk_recv, chunk_send };

        gen
    }

    pub fn queue_chunk(&mut self, pos: [i32; 3]) {
        let config = self.config.clone();
        let channel = self.chunk_send.clone();
        rayon::spawn(move || {
            let chunk = config.generate(pos);
            channel.send(chunk).unwrap();
        });
    }

    pub fn receive_chunks(&mut self) -> Vec<GeneratedChunk> {
        let vec = self.chunk_recv.try_iter().collect();

        vec
    }
}

struct CustomNoise<N> {
    noise: N,
    amp: f64,
    perm: f64,
}

impl<const DIM: usize, N: NoiseFn<f64, DIM>> NoiseFn<f64, DIM> for CustomNoise<N> {
    fn get(&self, point: [f64; DIM]) -> f64 { self.noise.get(point.map(|a| a * self.perm)) * self.amp }
}

impl<N> CustomNoise<N> {
    pub fn new(noise: N, amp: f64, perm: f64) -> CustomNoise<N> { Self { noise, amp, perm } }
}
