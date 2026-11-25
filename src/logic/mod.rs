//! Pure game rules. Nothing in this module touches rendering or input,
//! so it can be unit-tested on any host with `cargo test`.

pub mod ai;
pub mod battle;
pub mod campaign;
pub mod combat;
pub mod items;
pub mod map;
pub mod mapgen;
pub mod pathfinding;
pub mod rng;
pub mod supports;
pub mod terrain;
pub mod unit;

/// A tile coordinate on the battle map.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct Pos {
    pub x: i32,
    pub y: i32,
}

impl Pos {
    pub const fn new(x: i32, y: i32) -> Self {
        Pos { x, y }
    }

    pub fn manhattan(self, other: Pos) -> i32 {
        (self.x - other.x).abs() + (self.y - other.y).abs()
    }
}
