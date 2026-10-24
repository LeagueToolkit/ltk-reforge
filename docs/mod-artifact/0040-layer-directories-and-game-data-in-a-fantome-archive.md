# MOD-ARTIFACT-0040: Layer directories and game data in a Fantome archive

- **Source:** league-mod's `ltk_fantome` and `ltk_mod_project` crates, which LTK Manager reads
  and writes Fantome archives with
- **Affects:** the archive: its top-level directories and `META/info.json`
- **Related:** #MOD-ARTIFACT-0007, #ADR-0026, #ADR-0105

## What is found

A `.fantome` archive with entries outside the three directories the Fantome specification
defines (`META/`, `WAD/`, `RAW/`):

- `WAD_<layer>/<wad>`: the WADs of the layer named `<layer>`, in the two shapes `WAD/` takes,
  a packed `.wad.client` stored uncompressed or a directory of its files. `<layer>` is ASCII
  letters, digits, `-` and `_`, and is never `base`: the base layer's WADs stay in `WAD/`.
- `META/game_data/<layer>/<path>`: a layer's game-data override files, a PTCH among them.
- `META/hashes/<name>.hashes.txt`: hash tables the archive declares.

`META/info.json` carries them: `Layers`, keyed by layer name, each with `Name`, `Priority`,
`StringOverrides` and a `GameData` program; `Hashtables`, the tables under `META/hashes/`;
`Generator`, the writing tool and its version, such as `ltk_mod_project 0.17.1`. A packed WAD
is stored, not deflated, in every layer's WAD directory.

The archive is told from a plain Fantome by a `WAD_` directory, a `META/game_data/` or
`META/hashes/` entry, or any of the three fields in `info.json`.

## Why it is there

A mod project has layers and game-data programs, and a `.modpkg` stores every one of them. The
Fantome layout has one place for WAD content, which is the base layer's, and no place for a
program. league-mod's packer puts the rest where a reader of the specification alone does not
look, so one archive carries the whole project and still installs as its base layer in an older
manager. league-mod's own records state the choice (its ADR-0036, 2026-10-01) and the layout
(`crates/ltk_fantome/DESIGN.md`).

## What reads it, and how

- **LTK Manager** opens any zip as a Fantome, stores its packed WADs so they can be read in
  place, and reads a mod project out of `META/info.json`, giving a `WAD_<layer>/` directory the
  metadata does not declare a layer of its own at priority 0. Its layers and game data are the
  project's, as from a `.modpkg`. As it imports an archive it harvests the names its WADs
  spell and writes them into the archive as `META/hashes/game.hashes.txt`, declared under
  `Hashtables`: a plain Fantome in the manager's library carries that entry and nothing else of
  the extensions.
- **league-mod** packs a project to this layout and imports one from it.
- **cslol-manager and Fantome** mount every child of `WAD/`, pack `RAW/` into one WAD, copy
  `META/` whole and ignore every other top-level directory. The base layer installs as before.
  No other layer loads, and no game-data program runs: a PTCH under `META/game_data/` is copied
  and never applied.

## The repair

None: this is the form league-mod writes today.

## Evidence

- league-mod at commit e695f65 (2026-10-24): `crates/ltk_fantome/DESIGN.md` states the layout
  and names the readers that skip the layer directories; `crates/ltk_mod_project/src/fantome/pack.rs`
  writes every layer's WADs, override files and program. The crates are released as
  `ltk_fantome` 0.16.1 and `ltk_mod_project` 0.17.1 (its `fantome` feature).
- Two plain Fantomes imported by LTK Manager (2026-10-24): each archive in the manager's
  library holds `META/info.json` with a `Hashtables` field, `META/hashes/game.hashes.txt` and
  its `WAD/` entry, and no layer directory or game data; the library's index records the
  harvest's count beside each mod.
- LTK Manager at commit ac576af3 (2026-10-22) reads archives with `ltk_fantome` 0.15.5 and
  `ltk_mod_project` 0.16.7, both of which read the layer directories and the game data; its
  archive reader declares a layer for each `WAD_<layer>/` directory the metadata lacks. That its
  install applies a Fantome's game data was read off the library it uses, not off the manager's
  own pipeline (2026-10-24).
