# Emberfall Tactics

An original retro tactical RPG for Android (and desktop), written in Rust with [macroquad](https://macroquad.rs).
All art is pixel art drawn in code; no external assets.

**Milestone 1 – "The Ashford Crossing":** one complete battle on a 16×12 map. Five allies (Lord, Lancer, Archer, Mage, Cleric) face eight enemies led by the warlord Varga Ironjaw. Defeat Varga to win. If Kael, your lord, falls, the battle is lost.

## Rules at a glance

| | |
|---|---|
| Terrain | Plains · Forest (+1 Def, +20 Avo, cost 2) · Mountain (+2 Def, +30 Avo, cost 3; axe users 2) · Fort (+2 Def, +20 Avo, heals 20%/turn) · Water (impassable) · Bridge |
| Weapon triangle | Sword > Axe > Lance > Sword (±1 damage, ±15 hit) |
| Damage | Str/Mag + weapon Mt − Def/Res − terrain Def |
| Hit | Wpn Hit + 2·Skl + Lck/2 − (2·Spd + Lck + terrain Avo) |
| Crit | Wpn Crit + Skl/2 − foe Lck, triple damage |
| Doubling | Spd ≥ foe Spd + 4 → strike twice |
| Counters | Defender strikes back if the distance is within its weapon's range (bows: 2 only; tomes: 1–2; staves never) |
| XP | Hit ≈ 10, kill ≈ 30 (scaled by level gap), boss +40, heal 12. 100 XP = level up, rolled against growth rates |
| Phases | Player phase → enemy phase. Enemies attack the best target they can reach, otherwise advance; the boss holds his fort |
| Permadeath | Toggle on the title screen. On: fallen allies are lost forever. Off: they retreat and return later |

**Controls (touch or mouse):** tap a blue unit → tap a blue tile to move → choose **Attack / Heal / Wait / Cancel**.
Choosing Attack shows the combat forecast. Tap another red target to compare, then tap the target again or press **Attack!** to fight.
Tap an enemy to see its movement and attack range. **Enemy Range** shows every tile any enemy can threaten. **End Turn** ends your phase early.

## Project layout

```
src/logic/        pure game rules, no rendering (unit-tested)
  terrain.rs      terrain bonuses and movement costs
  unit.rs         classes, weapons, stats, growth rates, level-ups
  combat.rs       forecast + resolution formulas, XP
  pathfinding.rs  Dijkstra movement range, paths, attack ranges
  ai.rs           enemy decision making
  battle.rs       battle state: phases, actions, win/lose
  scenario.rs     the map and roster for the first battle
src/ui/           macroquad rendering and input
  gfx.rs          code-drawn pixel art (terrain, units)
  battle_screen.rs input state machine, animation queue, panel, forecast
  widgets.rs      layout, buttons, tap input
```

## Desktop

```sh
cargo run --release   # play
cargo test            # 46 unit tests (combat, pathfinding, AI, battle rules)
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
