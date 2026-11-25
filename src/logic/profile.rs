//! Player profile kept across campaigns: settings, lifetime counters,
//! achievements and the army carried into New Game+.

use serde::{Deserialize, Serialize};

use super::campaign::{Campaign, Difficulty};
use super::supports::MAX_LEVEL;

pub struct Achievement {
    pub id: &'static str,
    pub name: &'static str,
    pub desc: &'static str,
}

const fn a(id: &'static str, name: &'static str, desc: &'static str) -> Achievement {
    Achievement { id, name, desc }
}

pub const ACHIEVEMENTS: [Achievement; 16] = [
    a("first_victory", "First Light", "Win your first battle."),
    a("act1", "The Ashen King Falls", "Defeat Morvaine and finish Act I."),
    a("champion", "Flame Rekindled", "Finish the campaign."),
    a("ironman", "Iron Will", "Finish the campaign with permadeath on."),
    a("no_losses", "No One Left Behind", "Finish with permadeath on and nobody fallen."),
    a("hard", "Forged in Fire", "Finish the campaign on Hard."),
    a("recruiter", "Silver Tongue", "Recruit Dain, Ilse, Corvin, Rhea and Seren."),
    a("bonded", "Kindred Spirits", "Reach an A support."),
    a("promoted", "Rising Star", "Promote a unit with a Master Seal."),
    a("smith", "Master Smith", "Forge a weapon to +5."),
    a("arena", "Arena Champion", "Win 5 arena bouts."),
    a("treasure", "Treasure Hunter", "Open 10 chests."),
    a("pickpocket", "Light Fingers", "Steal 5 items."),
    a("skirmisher", "Seasoned Veterans", "Win 5 skirmishes."),
    a("rich", "Royal Treasury", "Hold 20,000 gold."),
    a("ngplus", "Once More, With Feeling", "Finish a New Game+ campaign."),
];

fn yes() -> bool {
    true
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Profile {
    #[serde(default = "yes")]
    pub music: bool,
    #[serde(default = "yes")]
    pub sfx: bool,
    #[serde(default)]
    pub unlocked: Vec<String>,
    #[serde(default)]
    pub arena_wins: u32,
    #[serde(default)]
    pub chests: u32,
    #[serde(default)]
    pub steals: u32,
    #[serde(default)]
    pub skirmish_wins: u32,
    #[serde(default)]
    pub promotions: u32,
    #[serde(default)]
    pub max_forge: u8,
    /// The last finished campaign, ready for New Game+.
    #[serde(default)]
    pub completed: Option<Campaign>,
}

impl Default for Profile {
    fn default() -> Self {
        Profile { music: true, sfx: true, unlocked: Vec::new(), arena_wins: 0, chests: 0, steals: 0, skirmish_wins: 0, promotions: 0, max_forge: 0, completed: None }
    }
}

impl Profile {
    pub fn has(&self, id: &str) -> bool {
        self.unlocked.iter().any(|u| u == id)
    }

    fn unlock(&mut self, id: &'static str, out: &mut Vec<&'static str>) {
        if !self.has(id) {
            self.unlocked.push(id.to_string());
            if let Some(a) = ACHIEVEMENTS.iter().find(|a| a.id == id) {
                out.push(a.name);
            }
        }
    }

    /// Checks every achievement against the campaign and counters. Returns
    /// the names of newly unlocked ones.
    pub fn check(&mut self, c: &Campaign) -> Vec<&'static str> {
        let mut out = Vec::new();
        if c.chapter >= 1 || c.finished {
            self.unlock("first_victory", &mut out);
        }
        if c.chapter >= 11 || c.finished {
            self.unlock("act1", &mut out);
        }
        if c.finished {
            self.unlock("champion", &mut out);
            if c.permadeath {
                self.unlock("ironman", &mut out);
                if c.fallen.is_empty() {
                    self.unlock("no_losses", &mut out);
                }
            }
            if c.difficulty == Difficulty::Hard {
                self.unlock("hard", &mut out);
            }
            if c.ng_plus > 0 {
                self.unlock("ngplus", &mut out);
            }
        }
        let names = ["Dain", "Ilse", "Corvin", "Rhea", "Seren"];
        if names.iter().all(|n| c.army.iter().any(|u| u.name == *n) || c.fallen.iter().any(|f| f == n)) {
            self.unlock("recruiter", &mut out);
        }
        if c.supports.iter().any(|s| s.level >= MAX_LEVEL) {
            self.unlock("bonded", &mut out);
        }
        let forge = c.army.iter().flat_map(|u| u.items.iter()).map(|i| i.forge).max().unwrap_or(0);
        self.max_forge = self.max_forge.max(forge);
        if self.max_forge >= super::items::MAX_FORGE {
            self.unlock("smith", &mut out);
        }
        if self.promotions >= 1 {
            self.unlock("promoted", &mut out);
        }
        if self.arena_wins >= 5 {
            self.unlock("arena", &mut out);
        }
        if self.chests >= 10 {
            self.unlock("treasure", &mut out);
        }
        if self.steals >= 5 {
            self.unlock("pickpocket", &mut out);
        }
        if self.skirmish_wins >= 5 {
            self.unlock("skirmisher", &mut out);
        }
        if c.gold >= 20_000 {
            self.unlock("rich", &mut out);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unlocks_once_and_reports_names() {
        let mut p = Profile::default();
        let mut c = Campaign::new(true);
        assert!(p.check(&c).is_empty());
        c.chapter = 1;
        assert_eq!(p.check(&c), vec!["First Light"]);
        assert!(p.check(&c).is_empty(), "only reported once");
        c.finished = true;
        c.difficulty = Difficulty::Hard;
        let got = p.check(&c);
        for n in ["Flame Rekindled", "Iron Will", "No One Left Behind", "Forged in Fire", "The Ashen King Falls"] {
            assert!(got.contains(&n), "{n} missing from {got:?}");
        }
    }

    #[test]
    fn counters_drive_achievements() {
        let mut p = Profile::default();
        let c = Campaign::new(false);
        p.arena_wins = 5;
        p.chests = 10;
        p.steals = 5;
        p.skirmish_wins = 5;
        p.promotions = 1;
        let got = p.check(&c);
        assert_eq!(got.len(), 5, "{got:?}");
    }

    #[test]
    fn ids_are_unique() {
        let mut ids: Vec<_> = ACHIEVEMENTS.iter().map(|a| a.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), ACHIEVEMENTS.len());
    }

    #[test]
    fn old_profiles_load() {
        let p: Profile = serde_json::from_str("{}").unwrap();
        assert!(p.music && p.sfx);
        assert!(p.unlocked.is_empty());
    }
}
