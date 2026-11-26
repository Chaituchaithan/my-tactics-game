#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Terrain {
    Plains,
    Forest,
    Mountain,
    Fort,
    Water,
    Bridge,
    Village,
    Throne,
    Wall,
    Floor,
    Pillar,
    Gate,
    /// Locked door: impassable until opened.
    Door,
    /// Treasure chest on a castle floor.
    Chest,
}

/// How a unit travels; determines terrain movement costs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveType {
    /// Standard foot soldiers.
    Infantry,
    /// Axe-wielding hill folk: cheaper mountains.
    Mountaineer,
    /// Horse riders: fast on open ground, slowed by forest, no mountains.
    Mounted,
    /// Pegasus riders: everything costs 1 except walls and doors.
    Flying,
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
            'V' => Terrain::Village,
            'H' => Terrain::Throne,
            '#' => Terrain::Wall,
            ',' => Terrain::Floor,
            'P' => Terrain::Pillar,
            'G' => Terrain::Gate,
            'D' => Terrain::Door,
            'C' => Terrain::Chest,
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
            Terrain::Village => "Village",
            Terrain::Throne => "Throne",
            Terrain::Wall => "Wall",
            Terrain::Floor => "Floor",
            Terrain::Pillar => "Pillar",
            Terrain::Gate => "Gate",
            Terrain::Door => "Door",
            Terrain::Chest => "Chest",
        }
    }

    pub fn defense(self) -> i32 {
        match self {
            Terrain::Forest | Terrain::Pillar => 1,
            Terrain::Mountain | Terrain::Fort => 2,
            Terrain::Throne | Terrain::Gate => 3,
            _ => 0,
        }
    }

    pub fn avoid(self) -> i32 {
        match self {
            Terrain::Village => 10,
            Terrain::Forest | Terrain::Fort | Terrain::Pillar | Terrain::Gate => 20,
            Terrain::Mountain | Terrain::Throne => 30,
            _ => 0,
        }
    }

    /// Percent of max HP restored at the start of the occupant's phase.
    pub fn heal_percent(self) -> i32 {
        match self {
            Terrain::Fort | Terrain::Throne => 20,
            Terrain::Gate => 10,
            _ => 0,
        }
    }

    /// Movement points needed to enter this tile, or None if impassable.
    pub fn move_cost(self, mt: MoveType) -> Option<i32> {
        match (self, mt) {
            (Terrain::Wall | Terrain::Door, _) => None,
            (_, MoveType::Flying) => Some(1),
            (Terrain::Water, _) => None,
            (Terrain::Mountain, MoveType::Mounted) => None,
            (Terrain::Forest, MoveType::Mounted) => Some(3),
            (Terrain::Pillar, MoveType::Mounted) => Some(3),
            (Terrain::Chest, _) => Some(1),
            (Terrain::Plains | Terrain::Bridge | Terrain::Village | Terrain::Floor | Terrain::Throne | Terrain::Gate, _) => Some(1),
            (Terrain::Forest | Terrain::Fort | Terrain::Pillar, _) => Some(2),
            (Terrain::Mountain, MoveType::Mountaineer) => Some(2),
            (Terrain::Mountain, _) => Some(3),
        }
    }
}
