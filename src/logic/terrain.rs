#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Terrain {
    Plains,
    Forest,
    Mountain,
    Fort,
    Water,
    Bridge,
}

/// How a unit travels; determines terrain movement costs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveType {
    /// Standard foot soldiers.
    Infantry,
    /// Axe-wielding hill folk: cheaper mountains.
    Mountaineer,
}

impl Terrain {
    pub fn from_char(c: char) -> Option<Terrain> {
        Some(match c {
            '.' => Terrain::Plains,
            'F' => Terrain::Forest,
            'M' => Terrain::Mountain,
            'T' => Terrain::Fort,
            '~' => Terrain::Water,
            '=' => Terrain::Bridge,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Terrain::Plains => "Plains",
            Terrain::Forest => "Forest",
            Terrain::Mountain => "Mountain",
            Terrain::Fort => "Fort",
            Terrain::Water => "Water",
            Terrain::Bridge => "Bridge",
        }
    }

    pub fn defense(self) -> i32 {
        match self {
            Terrain::Forest => 1,
            Terrain::Mountain | Terrain::Fort => 2,
            _ => 0,
        }
    }

    pub fn avoid(self) -> i32 {
        match self {
            Terrain::Forest | Terrain::Fort => 20,
            Terrain::Mountain => 30,
            _ => 0,
        }
    }

    /// Percent of max HP restored at the start of the occupant's phase.
    pub fn heal_percent(self) -> i32 {
        match self {
            Terrain::Fort => 20,
            _ => 0,
        }
    }

    /// Movement points needed to enter this tile, or None if impassable.
    pub fn move_cost(self, mt: MoveType) -> Option<i32> {
        match (self, mt) {
            (Terrain::Water, _) => None,
            (Terrain::Plains | Terrain::Bridge, _) => Some(1),
            (Terrain::Forest | Terrain::Fort, _) => Some(2),
            (Terrain::Mountain, MoveType::Infantry) => Some(3),
            (Terrain::Mountain, MoveType::Mountaineer) => Some(2),
        }
    }
}
