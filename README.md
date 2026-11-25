# Emberfall Tactics

An original retro tactical RPG for Android (and desktop), written in Rust with [macroquad](https://macroquad.rs).
All art is pixel art drawn in code; no external assets.

A complete 16-chapter campaign in two acts. Map size follows the story, from cramped passes to a whole capital city:

| # | Chapter | Map | Objective | Notes |
|---|---|---|---|---|
| 1 | The Ashford Crossing | Small | Defeat Varga | Hand-made map |
| 2 | Ember Woods | Small | Rout | Holt joins; talk to **Dain** |
| 3 | Hollowmere Must Stand | Small | Survive 6 turns | Sera joins |
| 4 | Saltmarsh Keep | Small | Seize | Talk to **Ilse** |
| 5 | The Drowned Road | Medium | Defeat Oskar | Marshland; Wren joins |
| 6 | Ironpeak Pass | Small | Defeat Thorne | Narrow mountain pass; Maeve joins |
| 7 | The Frozen Vigil | Medium | Seize | Monastery ruins; Aldous joins |
| 8 | The Ashen Wall | Large | Rout | Bram joins |
| 9 | The Burning Bridge | Large | Survive 7 turns | Talk to **Corvin** |
| 10 | Siege of Dawnhold | Extra Large | Defeat Kreth | The capital; talk to **Rhea** |
| 11 | Throne of Ash | Medium | Defeat Morvaine | End of Act I |
| 12 | The Frostmarch | Large | Survive 8 turns | Act II; Toma joins; enemies from all sides |
| 13 | Vault of Embers | Medium | Seize | Talk to **Seren** |
| 14 | The Shattered Coast | Extra Large | Rout | The Pale Fleet |
| 15 | The Hollow Bastion | Large | Defeat Ysolde | |
| 16 | Heart of Winter | Extra Large | Defeat Vaelith | Final battle |

Map sizes: Small 16×12, Medium 22×15, Large 28×18, Extra Large 34×22. Enemy numbers scale with the map, from about 8 to about 30. Chapters 5–16 use a deterministic map generator (`logic/mapgen.rs`). Each chapter describes a theme and its features (river and bridges, castle with gates/rooms/throne, forests, mountains, marsh, ruins, villages, forts) and always produces the same map. Tests check that every map can be played through.

**Camera:** drag the map to pan. Use **-/+** to switch between the whole map, the normal view and a close-up. The camera follows moving enemies, and tapping during the enemy phase fast-forwards it.

**Battle scenes:** fights play as a close-up side view, with lunges, arrows, spells, hit sparks, crit flashes and screen shake, damage numbers, and stat panels. Tap to skip. Turn them off with **Battle Scenes** on the preparations screen.

**Supports:** 10 pairs of allies (Kael & Brenna, Orrin & Ilse, Tamsin & Corvin and others) build bonds when they end turns within 2 tiles of each other. Each new level (C, B, A) unlocks a conversation in **Supports** on the preparations screen. In battle, each level gives +5 Hit, +5 Avoid, +2 Crit and +2 crit-dodge while the partner is within 3 tiles. The bonus is already included in the forecast.

**Art:** every sprite, portrait and tile is pixel art drawn in code: 32×32 units with outlines and 3-tone shading, varied hair and skin per character, 48×48 face portraits, and detailed terrain. Sprites are cached as textures.

Between chapters you get a **story scene** and a **preparations screen**: army and inventories (equip, discard, give items, use Master Seals, reorder who deploys) and an **armory** that sells weapons, staves and consumables. Progress **auto-saves** before each battle. Choose **Continue** on the title screen to resume.

## Rules at a glance

| | |
|---|---|
| Terrain | Plains, Forest (+1 Def, +20 Avo), Mountain (+2/+30), Fort (+2/+20, heals 20%), Village (visit for gifts), Castle floor, Pillar (+1/+20), Gate (+3/+20), Throne (+3/+30, heals), Water and Walls (impassable) |
| Weapon triangle | Sword > Axe > Lance > Sword (±1 damage, ±15 hit) |
| Damage | Str/Mag + weapon Mt − Def/Res − terrain Def |
| Hit | Wpn Hit + 2·Skl + Lck/2 − (2·Spd + Lck + terrain Avo) |
| Crit | Wpn Crit + Skl/2 − foe Lck, triple damage |
| Doubling | Spd ≥ foe Spd + 4 → strike twice |
| Counters | Only if the attacker is within the defender's weapon range (bows 2, javelins/hand axes/tomes 1–2, staves never) |
| Weapons | 27 items. Weapons have uses and break. Up to 5 items per unit. Tap **Weapon** in the forecast to switch |
| Classes | 10 base classes, plus 8 promoted ones at Lv 10+ with a Master Seal. Promoted classes gain stats, +1 Mov and a second weapon type |
| XP | Hit ≈ 10, kill ≈ 30 (scaled by level gap), boss +40, heal 12. 100 XP = level up, rolled against personal growth rates |
| AI | Aggressive units charge, guards wait for you to come into range, bosses hold their ground |
| Permadeath | Chosen at New Game. On: fallen allies are gone for good. Off: they return next chapter |

**Controls (touch or mouse):** tap a blue unit, then a blue tile to move. Then choose **Attack / Heal / Talk / Visit / Seize / Items / Wait / Cancel**.
Attack shows the combat forecast. Tap the target again (or **Attack!**) to fight. Tap an enemy to see its range. **Enemy Range** shows every tile under threat.

## Project layout

```
src/logic/        pure game rules, no rendering (unit-tested)
  terrain.rs      terrain bonuses and movement costs
  unit.rs         classes, weapons, stats, growth rates, level-ups
  combat.rs       forecast + resolution formulas, XP
  pathfinding.rs  Dijkstra movement range, paths, attack ranges
  items.rs        weapon/item catalogue and shop list
  ai.rs           enemy decision making
  battle.rs       battle state: phases, objectives, reinforcements, villages, talk, items
  campaign.rs     the 16 chapters (story, rosters, map recipes) and the saved campaign state
  mapgen.rs       deterministic map generator
src/ui/           macroquad rendering and input
  gfx.rs          code-drawn pixel art (terrain, units, portraits), cached as textures
  battle_screen.rs input state machine, animation queue, panel, forecast
  story.rs        dialogue scenes
  prep.rs         preparations: army, items, shop
  storage.rs      save file
  widgets.rs      layout, buttons, tap input
```

## Desktop

```sh
cargo run --release   # play
cargo test            # 80 tests: combat, pathfinding, AI, items, maps, supports, campaign
cargo test --release -- --ignored --nocapture   # auto-battle all 16 chapters
```

## Building the APK

Macroquad apps need **cargo-quad-apk**, the miniquad fork of cargo-apk. Plain `cargo apk` targets the
`android-activity` glue and does not work with macroquad. cargo-quad-apk needs an old toolchain
(JDK 8, build-tools with `dx`, an NDK that still ships `lib/gcc`), so the supported route is the
official Docker image `notfl3/cargo-apk`, which bundles all of it.

### Option A: locally with Docker

```sh
# one-time, if you don't have Docker Desktop:
brew install colima docker && colima start --arch x86_64 --memory 6

./scripts/build-apk.sh            # → dist/emberfall.apk
./scripts/build-apk.sh --install  # build and install over USB (adb)
```

The first build downloads the image (~2–3 GB). On Apple Silicon it runs emulated, so expect it to take a while.

### Option B: GitHub Actions

`.github/workflows/android.yml` runs the tests and then builds the APK on every push
(or manually from **Actions → Build APK → Run workflow**). Download `emberfall-apk` from the run's
**Artifacts** section. It is a zip that contains `emberfall.apk`.

## Installing on your phone

The APK is signed with a debug key, so you install it by sideloading rather than through the Play Store.

**Without a cable**

1. Copy `emberfall.apk` to the phone: Google Drive, email to yourself, or a USB file transfer.
2. Open it from the Files app (or the Downloads notification).
3. Android asks you to allow installs from that app (*Settings → Apps → Special app access → Install unknown apps*). Allow it, go back, and tap **Install**.
4. If Play Protect warns about an unknown developer, tap **More details → Install anyway**.

**With a cable (adb)**

1. On the phone, open *Settings → About phone* and tap **Build number** 7 times to enable Developer options.
2. Go to *Settings → System → Developer options* and turn on **USB debugging**.
3. Connect the phone and accept the "Allow USB debugging?" prompt.
4. Run `~/Library/Android/sdk/platform-tools/adb install -r dist/emberfall.apk`.

The game runs fullscreen in landscape.
