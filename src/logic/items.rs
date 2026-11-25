//! Item catalogue: weapons, staves and consumables (all original names).

use serde::{Deserialize, Serialize};

use super::unit::{Weapon, WeaponKind};

pub const MAX_ITEMS: usize = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ItemId {
    IronSword,
    SteelSword,
    SilverSword,
    KeenSword,
    Oathblade,
    IronLance,
    SteelLance,
    SilverLance,
    Javelin,
    IronAxe,
    SteelAxe,
    SilverAxe,
    HandAxe,
    Cleaver,
    ShortBow,
    SteelBow,
    Longbow,
    Ember,
    Blaze,
    Inferno,
    Gloom,
    Umbra,
    Mend,
    Renew,
    Vulnerary,
    Elixir,
    MasterSeal,
    DoorKey,
    ChestKey,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ItemKind {
    /// Weapons and staves (staves have kind `Staff`; their might is the heal bonus).
    Weapon(Weapon),
    /// Restores HP (99 = full).
    Heal(i32),
    /// Promotes a unit of level 10 or higher.
    Seal,
    Key(KeyKind),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyKind {
    Door,
    Chest,
}

/// Forge levels a weapon can take; each adds +1 Mt, +5 Hit, +3 Crit.
pub const MAX_FORGE: u8 = 5;

#[derive(Clone, Debug)]
pub struct ItemInfo {
    pub name: &'static str,
    pub uses: i32,
    pub price: i32,
    pub kind: ItemKind,
}

const fn w(name: &'static str, kind: WeaponKind, might: i32, hit: i32, crit: i32, min_range: i32, max_range: i32) -> Weapon {
    Weapon { name, kind, might, hit, crit, min_range, max_range }
}

impl ItemId {
    pub fn info(self) -> ItemInfo {
        use ItemId::*;
        use WeaponKind::*;
        let (name, uses, price, kind) = match self {
            IronSword => ("Iron Sword", 46, 460, ItemKind::Weapon(w("Iron Sword", Sword, 5, 90, 0, 1, 1))),
            SteelSword => ("Steel Sword", 30, 600, ItemKind::Weapon(w("Steel Sword", Sword, 8, 75, 0, 1, 1))),
            SilverSword => ("Silver Sword", 20, 1500, ItemKind::Weapon(w("Silver Sword", Sword, 13, 80, 0, 1, 1))),
            KeenSword => ("Keen Sword", 20, 1300, ItemKind::Weapon(w("Keen Sword", Sword, 4, 80, 25, 1, 1))),
            Oathblade => ("Oathblade", 60, 0, ItemKind::Weapon(w("Oathblade", Sword, 6, 95, 10, 1, 1))),
            IronLance => ("Iron Lance", 45, 360, ItemKind::Weapon(w("Iron Lance", Lance, 7, 80, 0, 1, 1))),
            SteelLance => ("Steel Lance", 30, 480, ItemKind::Weapon(w("Steel Lance", Lance, 10, 70, 0, 1, 1))),
            SilverLance => ("Silver Lance", 20, 1200, ItemKind::Weapon(w("Silver Lance", Lance, 14, 75, 0, 1, 1))),
            Javelin => ("Javelin", 20, 400, ItemKind::Weapon(w("Javelin", Lance, 6, 65, 0, 1, 2))),
            IronAxe => ("Iron Axe", 45, 270, ItemKind::Weapon(w("Iron Axe", Axe, 8, 75, 0, 1, 1))),
            SteelAxe => ("Steel Axe", 30, 360, ItemKind::Weapon(w("Steel Axe", Axe, 11, 65, 0, 1, 1))),
            SilverAxe => ("Silver Axe", 20, 1000, ItemKind::Weapon(w("Silver Axe", Axe, 15, 70, 0, 1, 1))),
            HandAxe => ("Hand Axe", 20, 300, ItemKind::Weapon(w("Hand Axe", Axe, 7, 60, 0, 1, 2))),
            Cleaver => ("Cleaver", 40, 0, ItemKind::Weapon(w("Cleaver", Axe, 11, 75, 10, 1, 1))),
            ShortBow => ("Short Bow", 45, 400, ItemKind::Weapon(w("Short Bow", Bow, 6, 85, 0, 2, 2))),
            SteelBow => ("Steel Bow", 30, 560, ItemKind::Weapon(w("Steel Bow", Bow, 9, 75, 0, 2, 2))),
            Longbow => ("Longbow", 20, 900, ItemKind::Weapon(w("Longbow", Bow, 7, 65, 0, 2, 3))),
            Ember => ("Ember", 40, 500, ItemKind::Weapon(w("Ember", Magic, 5, 90, 0, 1, 2))),
            Blaze => ("Blaze", 30, 800, ItemKind::Weapon(w("Blaze", Magic, 8, 80, 5, 1, 2))),
            Inferno => ("Inferno", 20, 1800, ItemKind::Weapon(w("Inferno", Magic, 13, 70, 10, 1, 2))),
            Gloom => ("Gloom", 40, 0, ItemKind::Weapon(w("Gloom", Magic, 6, 80, 5, 1, 2))),
            Umbra => ("Umbra", 60, 0, ItemKind::Weapon(w("Umbra", Magic, 14, 85, 10, 1, 2))),
            Mend => ("Mend Staff", 30, 600, ItemKind::Weapon(w("Mend Staff", Staff, 10, 100, 0, 1, 1))),
            Renew => ("Renew Staff", 15, 1200, ItemKind::Weapon(w("Renew Staff", Staff, 20, 100, 0, 1, 1))),
            Vulnerary => ("Vulnerary", 3, 300, ItemKind::Heal(10)),
            Elixir => ("Elixir", 1, 900, ItemKind::Heal(99)),
            MasterSeal => ("Master Seal", 1, 2500, ItemKind::Seal),
            DoorKey => ("Door Key", 1, 300, ItemKind::Key(KeyKind::Door)),
            ChestKey => ("Chest Key", 1, 400, ItemKind::Key(KeyKind::Chest)),
        };
        ItemInfo { name, uses, price, kind }
    }

    pub fn name(self) -> &'static str {
        self.info().name
    }

    pub fn weapon(self) -> Option<Weapon> {
        match self.info().kind {
            ItemKind::Weapon(w) => Some(w),
            _ => None,
        }
    }

    pub fn description(self) -> String {
        match self.info().kind {
            ItemKind::Weapon(w) if w.kind == WeaponKind::Staff => format!("Heals Mag+{} HP. Rng {}", w.might, w.range_label()),
            ItemKind::Weapon(w) => format!("{} Mt {} Hit {} Crt {} Rng {}", w.kind.name(), w.might, w.hit, w.crit, w.range_label()),
            ItemKind::Heal(99) => "Fully restores HP.".into(),
            ItemKind::Heal(n) => format!("Restores {} HP.", n),
            ItemKind::Seal => "Promotes a unit of Lv 10+.".into(),
            ItemKind::Key(KeyKind::Door) => "Opens one locked door.".into(),
            ItemKind::Key(KeyKind::Chest) => "Opens one treasure chest.".into(),
        }
    }
}

/// Things the armory sells between chapters.
pub const SHOP: &[ItemId] = &[
    ItemId::Vulnerary,
    ItemId::Elixir,
    ItemId::IronSword,
    ItemId::SteelSword,
    ItemId::IronLance,
    ItemId::SteelLance,
    ItemId::Javelin,
    ItemId::IronAxe,
    ItemId::SteelAxe,
    ItemId::HandAxe,
    ItemId::ShortBow,
    ItemId::SteelBow,
    ItemId::Ember,
    ItemId::Blaze,
    ItemId::Mend,
    ItemId::DoorKey,
    ItemId::ChestKey,
    ItemId::MasterSeal,
];

/// Extra stock unlocked as the campaign goes on.
pub fn shop_for(chapter: usize) -> Vec<ItemId> {
    let mut v = SHOP.to_vec();
    if chapter >= 6 {
        v.extend([ItemId::SilverSword, ItemId::SilverLance, ItemId::SilverAxe, ItemId::Longbow, ItemId::Renew]);
    }
    if chapter >= 10 {
        v.extend([ItemId::KeenSword, ItemId::Inferno]);
    }
    v
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    pub id: ItemId,
    pub uses: i32,
    /// Forge level (weapons only).
    #[serde(default)]
    pub forge: u8,
}

impl Item {
    pub fn new(id: ItemId) -> Item {
        Item { id, uses: id.info().uses, forge: 0 }
    }

    pub fn name(&self) -> String {
        if self.forge > 0 {
            format!("{}+{}", self.id.name(), self.forge)
        } else {
            self.id.name().to_string()
        }
    }

    pub fn label(&self) -> String {
        format!("{} {}/{}", self.name(), self.uses, self.id.info().uses)
    }

    /// Weapon stats including forge bonuses.
    pub fn weapon(&self) -> Option<Weapon> {
        let mut w = self.id.weapon()?;
        if w.can_attack() {
            let f = self.forge as i32;
            w.might += f;
            w.hit += 5 * f;
            w.crit += 3 * f;
        }
        Some(w)
    }

    pub fn can_forge(&self) -> bool {
        self.forge < MAX_FORGE && self.id.weapon().is_some_and(|w| w.can_attack())
    }

    /// Gold cost of the next forge level.
    pub fn forge_cost(&self) -> i32 {
        let base = self.id.info().price.max(600);
        base / 2 + 250 * (self.forge as i32 + 1)
    }
}
