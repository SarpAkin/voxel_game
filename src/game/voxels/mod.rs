mod chunk;
mod worldgen;

pub use chunk::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Tile(pub u16);

impl Tile {
    pub fn transparent(&self) -> bool { self.properties().is_transparent }
    pub fn properties(&self) -> &TileProperties { &properties[self.0 as usize] }
}

#[derive(Debug, Clone, Copy)]
pub struct TileProperties {
    pub name: &'static str,
    pub is_transparent: bool,
}

pub const properties: [TileProperties; 9] = [
    TileProperties { name: "air", is_transparent: true },
    TileProperties { name: "stone", is_transparent: false },
    TileProperties { name: "dirt", is_transparent: false },
    TileProperties { name: "grass", is_transparent: false },
    TileProperties { name: "sand", is_transparent: false },
    TileProperties { name: "glass", is_transparent: true },
    TileProperties { name: "wood", is_transparent: false },
    TileProperties { name: "leaf", is_transparent: true },
    TileProperties { name: "snow", is_transparent: false },
];

pub const AIR: Tile = Tile(0);
pub const STONE: Tile = Tile(1);
pub const GRASS: Tile = Tile(2);
pub const DIRT: Tile = Tile(3);
pub const SAND: Tile = Tile(4);
pub const SNOW: Tile = Tile(8);
