use super::terrain::Terrain;
use super::Pos;

#[derive(Clone, Debug)]
pub struct Map {
    pub width: i32,
    pub height: i32,
    tiles: Vec<Terrain>,
}

impl Map {
    /// Builds a map from rows of terrain characters (see `Terrain::from_char`).
    pub fn from_rows(rows: &[&str]) -> Map {
        let height = rows.len() as i32;
        let width = rows.first().map(|r| r.chars().count()).unwrap_or(0) as i32;
        let mut tiles = Vec::with_capacity((width * height) as usize);
        for (y, row) in rows.iter().enumerate() {
            assert_eq!(row.chars().count() as i32, width, "row {y} has wrong width");
            for c in row.chars() {
                tiles.push(Terrain::from_char(c).unwrap_or_else(|| panic!("bad terrain char {c:?}")));
            }
        }
        Map { width, height, tiles }
    }

    /// A uniform map, handy for tests.
    #[cfg(test)]
    pub fn filled(width: i32, height: i32, t: Terrain) -> Map {
        Map { width, height, tiles: vec![t; (width * height) as usize] }
    }

    pub fn in_bounds(&self, p: Pos) -> bool {
        p.x >= 0 && p.y >= 0 && p.x < self.width && p.y < self.height
    }

    pub fn get(&self, p: Pos) -> Option<Terrain> {
        if self.in_bounds(p) {
            Some(self.tiles[(p.y * self.width + p.x) as usize])
        } else {
            None
        }
    }

    /// Terrain at `p`; out-of-bounds reads as Plains (callers bounds-check first).
    pub fn at(&self, p: Pos) -> Terrain {
        self.get(p).unwrap_or(Terrain::Plains)
    }


    pub fn neighbors(&self, p: Pos) -> impl Iterator<Item = Pos> + '_ {
        [(1, 0), (-1, 0), (0, 1), (0, -1)]
            .into_iter()
            .map(move |(dx, dy)| Pos::new(p.x + dx, p.y + dy))
            .filter(move |n| self.in_bounds(*n))
    }
}
