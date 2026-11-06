# AGENTS.md

Guidance for agents working in this repository. Read `README.md` first for the
user-facing overview; this file records what is *not* obvious from a single file.

## What this project is

Procedural generation of LiDAR scenes and point clouds (terrain, water, plants,
buildings, whole 100x100 m terrains) plus a scanner simulator. Rust 2024 edition
workspace, Bevy 0.20 for visualisation.

Every generator produces three things from the same source data:

1. pure generation data (no rendering),
2. polygon meshes for the visual mode,
3. a `PointCloud` with `PointClass` labels (LAS-like codes).

Generation is **fully deterministic from a `Seed`**; identical preset + seed must
produce identical geometry and point clouds. `wiki/Архитектура.md` and
`wiki/Создание объекта.md` describe the intended pipeline and entity hierarchy.

## Commands

There is no CI, Makefile, `rustfmt.toml`, `clippy.toml` or custom cargo aliases.
Only `cargo` subcommands from the README/wiki are supported:

```sh
cargo test --workspace
cargo check --workspace
```

`cargo test --workspace` now also compiles `lidar-dataset-runner`, so it needs
the WebKitGTK system dev packages (`libwebkit2gtk-4.1-dev`, `libsoup-3.0-dev`)
and a built or stubbed `crates/lidar-dataset-runner/ui/dist`. Without them, run
the other crates by name:

```sh
cargo test -p generation-core -p landscapes-generator -p buildings-generator \
  -p plants-generator -p terrains-generator
cargo test -p lidar-dataset-runner
```

Running the apps (each from its own crate directory, see assets gotcha below):

```sh
cd crates/landscapes-generator && cargo run
cd crates/buildings-generator  && cargo run            # -- --preset rectangular|l-shaped|annex|apartment|skyscraper
cd crates/plants-generator     && cargo run
cd crates/terrains-generator   && cargo run -- village 42
cd crates/lidar-simulator      && cargo run            # window mode
```

`lidar-simulator` also has a headless exporter (see `--help`):

```sh
cargo run -p lidar-simulator -- --headless --source-step 0.06 --scan-step 0.2 \
  --noise-probability 0.02 --noise-extent 0.12 --frames 72 \
  --seed 12648430 --output assets/export/building-scan.ply
```

`terrains-generator` parses positional args itself: `[preset] [seed]`, plus
`--list`. Unlike the other generators it has no `--preset` flag.

Regenerating the demo preset catalogue (examples, not tests). `terrains` depends
on sub-presets from the other three categories, so run it **last**:

```sh
cargo run -p landscapes-generator --example dump_presets
cargo run -p buildings-generator  --example dump_presets
cargo run -p plants-generator     --example dump_presets          # skips existing; add --force to rewrite
cargo run -p terrains-generator   --example dump_presets
cargo run -p plants-generator     --example inspect_presets 42    # diagnostic dump
```

### Build profile is deliberate

`[profile.dev]` keeps `opt-level = 0` for the workspace **and** for dependencies.
This is not an oversight: `opt-level = 3` for dependencies made even `cargo check`
take ~5.5 minutes and grew `target/` to 23 GB. Do not "optimise" the dev profile;
use `--release` for demos.

## Toolchain and API versions (easy to get wrong)

- `rust-toolchain.toml` pins `stable`; `rust-version = "1.97.1"`, edition 2024.
- Bevy **0.20** — recent API, not the commonly memorised 0.14/0.15 one:
  - messages, not events: `#[derive(Message)]`, `app.add_message::<T>()`,
    `MessageReader<T>` / `MessageWriter<T>` (there is no `EventReader`);
  - `Mesh3d(handle)` / `MeshMaterial3d(handle)` components;
  - `GlobalAmbientLight { brightness, .. }`, `DirectionalLight::shadow_maps_enabled`;
  - `TextFont { font: handle.into(), font_size: FontSize::Px(15.0), .. }`;
  - mesh types live under `bevy::mesh` / `bevy::pbr`.
- rand **0.10**: use `rand::RngExt` and `rng.random()`, `rng.random_range(a..=b)`.
  Older `Rng::gen`/`gen_range` do not exist here.
- `ron 0.12`, `noise 0.9`, `thiserror 2`.
- Generators enable Bevy's `serialize` feature (`bevy = { workspace = true, features = ["serialize"] }`).

## Crate layout and dependency direction

```
generation-core  <- bevy-utils <- {landscapes, buildings, plants}-generator
                                         ^
                       terrains-generator (composes all three)
                                         ^
                       lidar-simulator (buildings + generation-core)
                       lidar-dataset-runner (all generators + terrains-generator, Tauri app)
```

- `generation-core`: `Seed`/`GlobalSeed`, `Occupancy`, `Polygon`, `PointCloud` /
  `PointClass` / XYZ export, `PresetStore` and preset RON plumbing, shared
  hotkeys (`V`, `R`, `Shift+R`) and `UiModalOpen`.
- `bevy-utils`: cameras (`Tab`), UI window shell, mesh/material helpers, preset
  selector window (`Ctrl+S`).
- Generators (landscapes, buildings, plants) share one internal layout:
  - `src/generation/` — pure data, usable headless, no rendering;
  - `src/visual/` — meshes and point-cloud entities;
  - `src/plugin.rs` — Bevy plugin wiring the scene, info panel, regeneration;
  - `src/main.rs` — thin binary assembling plugins;
  - `src/tests.rs` — unit tests, included via `#[cfg(test)] mod tests;` in `lib.rs`.
- For headless use, call the `generation` module directly rather than the plugin.

### terrains-generator differs

It has a flat layout (`preset.rs`, `placement.rs`, `scene.rs`, `viewer.rs`,
`main.rs`) and duplicates the visualisation by reusing the other crates'
`spawn_*` functions; it does not own rendering code.

`crates/terrains-generator/src/terrain.rs` is an **orphaned file** that `lib.rs`
does not declare (it references a non-existent `TerrainSettings` and would not
compile if added). The live implementation is `scene.rs` + `placement.rs`.

### lidar-dataset-runner differs

It is the only Tauri app in the workspace: `tauri.conf.json`, `capabilities/`
and `icons/` sit next to `Cargo.toml` (no `src-tauri/` — the workspace glob
would not pick a nested crate up), and the Vue frontend lives in `ui/`.

- `src/dataset/` is the pure logic (catalogue, recipe, scene composition,
  cloud merge, statistics, export, batch loop) and **must not** depend on
  `AppHandle`: it is driven by an `FnMut(&ProgressEvent)` callback and an
  `AtomicBool` cancel flag, which is what makes it testable with plain
  `cargo test`.
- `src/commands.rs` are thin Tauri adapters; generation always runs in
  `tauri::async_runtime::spawn_blocking`, never on the UI thread.
- `ui/dist/index.html` is a committed stub so `cargo test`/`cargo check` work
  before `npm run build`; real output is gitignored.
- `presets/` is bundled as a resource (`bundle.resources`), and the app finds
  it via `LIDAR_PRESETS_ROOT` → `resource_dir()/presets` → `resource_dir()` →
  the workspace catalogue.
- Scene composition **deliberately duplicates** `terrains_generator`'s
  placement order and constants (buildings → trees → bushes → grass) because
  the terrain composer is tied to `TerrainPresetData`. Only `PlacementMap` and
  `PlacementRules` are shared; keep the two in step when placement changes.

## Coordinate systems (a recurring source of bugs)

- Generator data (`Landscape`, `Part`, `Plant`, `TerrainScene`) is **Z-up**;
  horizontal maps are XY.
- Visualisation nests everything under a root rotated by
  `Quat::from_rotation_x(-FRAC_PI_2)` so local Z maps to world Y.
- `lidar-simulator` works in **world Y-up** and applies that same -90° X
  rotation itself in `ScanScene::from_building`.

When moving geometry between crates, check which convention the API expects.

### Entity hierarchy

Per `wiki/Создание объекта.md`: root entity with a marker component → one entity
per part (may nest) → child entity holding the `PointCloud` component. Visual
entities carry the `VisualMesh` marker, point clouds carry `PointCloudEntity`;
both marker types exist per crate and are used by the `V` toggle. `Transform` is
required even in headless mode because placement uses it, and placed objects
carry `Occupancy` on the root entity.

## Determinism rules

- Never call `Seed::from_entropy()`, `rand::random()`, `thread_rng()` or use
  system time inside generation code. The only legitimate entropy source is the
  `R` hotkey ("new random scene").
- Derive independent streams with `Seed::child(label: u64)` or
  `Seed::with_label("name")` (SplitMix64 / FNV-1a); create an RNG with
  `seed.rng()`.
- Tests should assert determinism: same seed → byte-identical results.

## Presets (`presets/`)

Directory rules (`generation-core/src/presets/mod.rs`):

- A directory is a **preset** when `manifest.ron` exists without `is_group: true`
  and `config.ron` exists. With `is_group: true` — a **group**. Without a
  manifest — a group (legacy form). Groups nest arbitrarily.
- Identity comes from `manifest.ron`'s `id` and `kind`, **not** the folder name.
  Folders may be renamed or moved between groups; don't "fix" code to match paths.
- `kind` is a plain string (not `Option`) and is resolved by tag via
  `PresetStore::by_kind` or `resolve_cross_category(category, id, presets_root)`.
- `config.ron` is the generator config: `LandscapeConfig`, `PlantConfig`,
  `BuildingConfig` / `TerrainPresetData`. `TerrainPresetData` stores the ids of
  sub-presets from other categories, so terrains only work if those exist.
- Landscapes are special: their `config.ron` holds baked geometry from
  `resolve(archetype, Seed::new(0))`, but the plugin rebuilds the archetype from
  `kind` for the current seed so `R` actually varies the terrain. Only
  non-archetype `kind`s fall back to the file. Do not assume the file's shape is
  what gets rendered.
- The preset selection is stored in `crates/<crate>/settings/preset-selection.ron`
  (gitignored). No file → everything selected. Closing the picker without
  "Применить" reverts; applying triggers regeneration with the same seed.
- `dump_presets` examples overwrite manifests/configs unconditionally, except
  plants, which skip existing folders unless `--force`.

Generated output is gitignored: `*.xyz|*.las|*.laz|*.ply|*.pcd`,
`crates/*/assets/export/`, `crates/*/settings/`, `local/`, `wiki/local/`.

## Bevy app conventions

- Hotkey systems must early-return while a modal is open: check `Res<UiModalOpen>`
  (`if ui.0 { return; }`). The preset window sets it while `Ctrl+S` is active.
- Do not hard-code paths relative to the process cwd for presets: binaries derive
  the workspace root from `env!("CARGO_MANIFEST_DIR").join("../..")`.
- Assets do not: `AssetServer` loads `fonts/Roboto-Regular.ttf` relative to the
  current directory, and each generator has its own `assets/fonts/`. Run apps
  from their crate directory (this is why README instructs `cd crates/<crate>`).
- Systems routinely need `#[allow(clippy::too_many_arguments)]`; that is the norm
  for Bevy systems here and not a smell.
- `PresetStore` is an optional `Option<Res<PresetStore>>` in generator plugins so
  they still work without the preset selector plugin — keep that fallback.

## Data format details

- `PointClass::code()` values are a compatibility contract used by both XYZ and
  PLY exports: `Other=1, Ground=2, Vegetation=5, Building=6, LowNoise=7, Water=9`.
  Do not renumber.
- `generation-core/src/point-clouds/export.rs` writes `.xyz` as
  `class x y z` per line (no header).
- `lidar-simulator/src/demo.rs` writes ASCII PLY with an `uchar classification`
  field.
- `PointClass::default()` is `Building`, not `Ground` — set the default class
  explicitly when building a cloud.
- `Seed` is a newtype over `u64`, so in RON it appears as `seed: (12648430)`.
- Module directory `point-clouds` uses a hyphen, so `generation-core/src/lib.rs`
  declares it with `#[path = "point-clouds/mod.rs"] pub mod point_clouds;`.
  Match that pattern for any new hyphenated directory.

## Style conventions

- Doc comments (`//!`, `///`) and user-facing strings are mostly **Russian**;
  identifiers and RON keys are English. `terrains-generator` and
  `lidar-simulator` are written in English (mixed within the crate). Match the
  file you are editing rather than converting language.
- Module-level `//!` docs explain *why* the algorithm is shaped the way it is;
  new non-trivial modules are expected to follow this.
- Preset `name`/`description` fields and info-panel strings are Russian.
- Test names are descriptive English snake_case; there are 100+ tests, unit tests
  inline in `generation-core` modules and generator tests in `src/tests.rs`.
- Test fixtures write under `target/<name>` (e.g. `target/preset-tests`), because
  the system temp directory may not be writable in the dev environment. Clean up
  with `remove_dir_all` at the end.
- One test is `#[ignore = "manual: требует сгенерированного дерева presets/"]`;
  it requires the preset catalogue to be dumped first.

## Documentation

- `README.md` is the user-facing source of truth for commands, hotkeys and preset
  counts; update it when CLI flags, presets or controls change.
- `wiki/` is an Obsidian vault with team process docs; links between notes use
  `[[name]]` and must stay consistent if files are renamed.
