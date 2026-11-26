//! Support bonds: allies who fight side by side earn support points, unlock
//! conversations (C, B, A) between chapters, and gain hit/avoid/crit when
//! near each other in battle.

use serde::{Deserialize, Serialize};

use super::campaign::Line;
use super::combat::Bonus;

/// Points needed for C, B and A.
pub const THRESHOLDS: [u32; 3] = [3, 8, 14];
pub const MAX_LEVEL: u8 = 3;
/// Partners within this many tiles grant their bonus.
pub const RANGE: i32 = 3;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupportState {
    pub a: String,
    pub b: String,
    pub points: u32,
    pub level: u8,
}

impl SupportState {
    pub fn new(a: &str, b: &str) -> Self {
        SupportState { a: a.into(), b: b.into(), points: 0, level: 0 }
    }

    pub fn involves(&self, name: &str) -> bool {
        self.a == name || self.b == name
    }

    pub fn partner(&self, name: &str) -> Option<&str> {
        if self.a == name {
            Some(&self.b)
        } else if self.b == name {
            Some(&self.a)
        } else {
            None
        }
    }

    pub fn is(&self, x: &str, y: &str) -> bool {
        (self.a == x && self.b == y) || (self.a == y && self.b == x)
    }

    /// A new conversation can be viewed.
    pub fn ready(&self) -> bool {
        self.level < MAX_LEVEL && self.points >= THRESHOLDS[self.level as usize]
    }

    pub fn gain(&mut self, n: u32) {
        let cap = THRESHOLDS[(MAX_LEVEL - 1) as usize];
        self.points = (self.points + n).min(cap);
    }
}

pub fn level_name(level: u8) -> &'static str {
    match level {
        1 => "C",
        2 => "B",
        3 => "A",
        _ => "-",
    }
}

/// Bonus one partner of `level` grants.
pub fn bonus_for(level: u8) -> Bonus {
    let l = level as i32;
    Bonus { hit: 5 * l, avoid: 5 * l, crit: 2 * l, dodge: 2 * l }
}

pub fn add(a: Bonus, b: Bonus) -> Bonus {
    Bonus { hit: (a.hit + b.hit).min(30), avoid: (a.avoid + b.avoid).min(30), crit: (a.crit + b.crit).min(15), dodge: (a.dodge + b.dodge).min(15) }
}

pub struct PairDef {
    pub a: &'static str,
    pub b: &'static str,
    /// C, B and A conversations.
    pub talks: [Vec<Line>; 3],
}

const fn l(speaker: &'static str, text: &'static str) -> Line {
    Line { speaker, text }
}

pub fn pairs() -> Vec<PairDef> {
    vec![
        PairDef {
            a: "Kael",
            b: "Brenna",
            talks: [
                vec![
                    l("Brenna", "You took that hit for me back there. Don't do that again, my lord."),
                    l("Kael", "You'd have done the same."),
                    l("Brenna", "That's my job. Yours is to stay alive."),
                ],
                vec![
                    l("Kael", "Brenna, how long have you served my family?"),
                    l("Brenna", "Since before you could hold a wooden sword. I taught you to fall without breaking your wrist."),
                    l("Kael", "I remember. I fell a lot."),
                    l("Brenna", "You still do. You just get back up faster now."),
                ],
                vec![
                    l("Brenna", "When this is over... the March will need a lord. Not a soldier."),
                    l("Kael", "It'll need both. I'd like you to command the guard."),
                    l("Brenna", "...Then I'd better keep you alive a while longer. Thank you, Kael."),
                ],
            ],
        },
        PairDef {
            a: "Kael",
            b: "Liese",
            talks: [
                vec![
                    l("Liese", "Hold still. You've cut your arm open again."),
                    l("Kael", "It's only a scratch."),
                    l("Liese", "Everything is 'only a scratch' with you."),
                ],
                vec![
                    l("Kael", "Do you ever regret leaving the Dawn Chapel?"),
                    l("Liese", "Sometimes. It was quiet there. But the Flame isn't in a chapel any more. It's out here, with us."),
                ],
                vec![
                    l("Liese", "Kael. Whatever we find in the north, promise me you won't face it alone."),
                    l("Kael", "I haven't been alone since Ashford. I'm not starting now."),
                    l("Liese", "Good. Then I'll stay right behind you."),
                ],
            ],
        },
        PairDef {
            a: "Orrin",
            b: "Ilse",
            talks: [
                vec![
                    l("Ilse", "You still hold your tome upside down when you're nervous."),
                    l("Orrin", "I do not! ...Do I?"),
                ],
                vec![
                    l("Orrin", "I thought you'd joined the Legion willingly. I said terrible things about you."),
                    l("Ilse", "I heard. You always were dramatic. I forgive you."),
                ],
                vec![
                    l("Ilse", "When the College reopens, they'll need two new masters of fire."),
                    l("Orrin", "Two? You mean... together?"),
                    l("Ilse", "Don't make me say it twice, Orrin."),
                ],
            ],
        },
        PairDef {
            a: "Tamsin",
            b: "Corvin",
            talks: [
                vec![
                    l("Tamsin", "You missed me on purpose at the bridge. Twice."),
                    l("Corvin", "Three times. You didn't notice the third."),
                ],
                vec![
                    l("Corvin", "Your draw is fast, but you close your left eye. You lose the wind that way."),
                    l("Tamsin", "And you breathe out too early. Shall we trade lessons?"),
                ],
                vec![
                    l("Tamsin", "Best of ten, at the far target. Loser cooks."),
                    l("Corvin", "You know I can't cook."),
                    l("Tamsin", "Then you'd better not lose."),
                ],
            ],
        },
        PairDef {
            a: "Holt",
            b: "Toma",
            talks: [
                vec![
                    l("Toma", "Southerner! You swing that axe like you're chopping firewood."),
                    l("Holt", "I was a miller. Firewood is what I know."),
                ],
                vec![
                    l("Holt", "Your clans really fight bears for sport?"),
                    l("Toma", "Only small ones. The big ones we just talk to, very politely."),
                ],
                vec![
                    l("Toma", "When the war ends, come north. We'll build you a mill on the frozen river."),
                    l("Holt", "A mill on ice. That'd be a sight. ...I'd like that, friend."),
                ],
            ],
        },
        PairDef {
            a: "Sera",
            b: "Bram",
            talks: [
                vec![
                    l("Bram", "You keep your shield too high, girl. Leaves your knees open."),
                    l("Sera", "And you keep yours too low, sir. Leaves your pride open."),
                ],
                vec![
                    l("Sera", "Were you really at the Siege of Graymoor?"),
                    l("Bram", "I was. Held the gate for three days. Most of the songs get the number wrong."),
                ],
                vec![
                    l("Bram", "I'm getting old, Sera. The March will need a new shield."),
                    l("Sera", "Then teach me everything, while there's still time."),
                    l("Bram", "Ha! Starting with your knees."),
                ],
            ],
        },
        PairDef {
            a: "Dain",
            b: "Maeve",
            talks: [
                vec![
                    l("Maeve", "A sellsword with a conscience. Didn't know those existed."),
                    l("Dain", "We're rare. Like honest guides."),
                ],
                vec![
                    l("Dain", "Why'd you really stay in the mountains all those years?"),
                    l("Maeve", "No one there asks questions. Like you're doing now."),
                ],
                vec![
                    l("Maeve", "After this, I'm going to see the sea. Never have."),
                    l("Dain", "I know a coast road. Could use a guide, if you're offering."),
                    l("Maeve", "...I suppose I am."),
                ],
            ],
        },
        PairDef {
            a: "Liese",
            b: "Wren",
            talks: [
                vec![
                    l("Wren", "Liese, you're bleeding! Let me heal you for once."),
                    l("Liese", "Wren, I'm fine, really... Oh. Thank you."),
                ],
                vec![
                    l("Liese", "You used to hide in the bell tower to skip morning prayers."),
                    l("Wren", "And you always brought me bread so I wouldn't starve!"),
                ],
                vec![
                    l("Wren", "When we rebuild the Chapel, I want you to lead it."),
                    l("Liese", "Only if you promise to come to morning prayers."),
                    l("Wren", "...Most mornings."),
                ],
            ],
        },
        PairDef {
            a: "Aldous",
            b: "Seren",
            talks: [
                vec![
                    l("Aldous", "The Vault's keeper, walking beside a Vigil abbot. The chroniclers would faint."),
                    l("Seren", "Then we must keep better chroniclers."),
                ],
                vec![
                    l("Seren", "Your monastery kept the histories. Mine kept the Heart. Neither of us kept the whole truth."),
                    l("Aldous", "Then perhaps we should write it down together."),
                ],
                vec![
                    l("Aldous", "When this is over, will you come to the Vigil? There's a library with an empty desk."),
                    l("Seren", "Four hundred years alone in a vault. A desk with company sounds... wonderful."),
                ],
            ],
        },
        PairDef {
            a: "Rhea",
            b: "Kael",
            talks: [
                vec![
                    l("Rhea", "My children talk about you constantly. 'The lord who saved Mama.'"),
                    l("Kael", "Maeve and Dain did the saving."),
                    l("Rhea", "They'll get their own stories. You're the bedtime one."),
                ],
                vec![
                    l("Kael", "Dawnhold will need its guard captain back when we're done."),
                    l("Rhea", "Dawnhold will need a king. Have you thought about that?"),
                ],
                vec![
                    l("Rhea", "I served three kings. You're the first one I'd follow into the cold."),
                    l("Kael", "I'm not a king, Rhea."),
                    l("Rhea", "Not yet."),
                ],
            ],
        },
    ]
}

pub fn conversation(a: &str, b: &str, level: u8) -> Option<Vec<Line>> {
    let p = pairs().into_iter().find(|p| (p.a == a && p.b == b) || (p.a == b && p.b == a))?;
    p.talks.get(level as usize).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_unlock_in_order() {
        let mut s = SupportState::new("Kael", "Brenna");
        assert!(!s.ready());
        s.gain(3);
        assert!(s.ready());
        s.level = 1;
        assert!(!s.ready());
        s.gain(100);
        assert_eq!(s.points, THRESHOLDS[2], "points are capped");
        assert!(s.ready());
        s.level = 3;
        assert!(!s.ready(), "A is the last level");
    }

    #[test]
    fn bonuses_stack_with_caps() {
        let b = add(bonus_for(3), bonus_for(3));
        assert_eq!(b.hit, 30);
        assert_eq!(b.crit, 12);
        assert_eq!(bonus_for(0), Bonus::default());
    }

    #[test]
    fn every_pair_has_three_conversations() {
        let ps = pairs();
        assert!(ps.len() >= 8);
        for p in &ps {
            for t in &p.talks {
                assert!(t.len() >= 2);
                for line in t {
                    assert!(line.speaker == p.a || line.speaker == p.b, "{} speaks in {}-{}", line.speaker, p.a, p.b);
                }
            }
            assert!(conversation(p.b, p.a, 2).is_some());
        }
    }
}
