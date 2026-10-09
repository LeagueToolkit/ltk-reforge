<p align="center">
  <img src="crates/reforge-app/icons/128x128.png" width="96" alt="">
</p>

<h1 align="center">ltk-reforge</h1>

<p align="center">
  A fixer and a porter for League of Legends skin mods.<br>
  It reads a mod made for an older patch and writes one the current patch loads.
</p>

<p align="center">
  <img alt="work in progress" src="https://img.shields.io/badge/status-%F0%9F%9A%A7%20work%20in%20progress-orange">
  <img alt="license GPL-3.0-or-later" src="https://img.shields.io/badge/license-GPL--3.0--or--later-blue">
  <img alt="rust 1.88" src="https://img.shields.io/badge/rust-1.88-black">
</p>

> 🚧 **Work in progress.** The fixer runs over the whole corpus and a handful of its packages
> are confirmed in game. The app is released as v0.1.4 (2026-10-09), and every format and
> command below may still change.

## What it does

Riot changes the game's data formats a few times a year. A field is renamed, a value changes
type, a struct is split or nested, a texture path moves. A skin mod made for patch 11.x stores
its data in the shapes of patch 11.x, and the current client reads garbage from it, or crashes.
Authors move on, and the mod stays broken.

reforge takes one mod archive through nine stages and writes a package the current build reads:

```
archive bytes
   │ read      the untrusted zip, in memory, under caps
   │ load      WADs, files, bins, objects, characters, skin units; what the client loads
   │ fix       values as the target build reads them, with a record of every change
   │ facts     what is true of the mod: copies, provenance, origin, rig, clips
   │ decide    which skin each unit is for
   │ plan      what ships, renamed and placed; audio; HUD
   │ build     layers, chunks, patches, manifests, hash tables
   │ check     the package applied as the overlay applies it; types
   ▼ write     a project directory, a .modpkg
```

Two programs run on those stages:

- **`reforge repair`**, the fixer: the mod as it is, at its own slots and paths, with its
  files and values fixed for the current build.
- **`reforge port`**, the porter: the fixed mod put over the game's own skin it was made for,
  as a patch of the game's bin with the mod's own objects and files under a namespace of the
  mod's.

A run never runs anything from a mod and never extracts a mod to disk. The archive is read in
memory, and only what the run derived is written.

## Why

The mods in the datasets this was built against are nearly all one kind: of about 6,000
archives dumped from mod sites, 4,952 are champion skin mods, 4,909 of them held in the
datasets (the rest are maps, UI, audio-only packs, chroma VFX packs, restorations of a
champion's older content, and archives that are not a mod at all). A skin mod is a known
shape: one or more
`skin<N>.bin`, a mesh, a skeleton, clips, textures, effects and sounds, all of it describing
one look of one champion. That shape is what makes fixing and porting tractable, and what makes
it worth doing for thousands of mods at once rather than one at a time by hand.

### The `.fantome` format is a bag of files

A `.fantome` is a zip of WADs and a `META/info.json`. It says nothing about what the mod is
for: which champion, which skin, which build it was made at, which files are the author's and
which are copies of the game's. A mod manager mounts the WADs over the game's and hopes.

The `.modpkg` format reforge writes is the one LTK Manager reads. A package is layers a player
turns on and off (the model; voice; HUD), chunks named by path, patches that change fields of
the game's own bin instead of replacing the whole bin, and a manifest. reforge also embeds the
evidence of the run that made the package: the build the mod dates to, what it restored, what
it changed and why. A package repaired again reads that evidence instead of guessing.

### What is in a mod

Because the archive is a bag of files, authors and their tools put anything in it, and a mod
manager mounts all of it. reforge
mounts none of it. A run parses the archive into its own representation of the mod: the
objects it reads from the bins, the files those objects name, the characters and skins they
describe. The package is then built from that representation, not from the archive. A file
nothing names is left out, a bin is written back field by field from what was parsed, an
executable or a nested archive never reaches the output, and nothing from the archive is
extracted or run on the way. A port is safe by construction: whatever was in the archive, the
package holds only what a skin needs, in bytes reforge wrote.

From the baseline run over the 5,137 mods (2026-10-03):

| what | how many mods |
|---|---:|
| the WAD packed whole as one zip entry, not as a folder of named files | 4,932 |
| at least one chunk no hash table spells | 3,110 |
| sources in the archive: ritobin text or JSON of a bin, Maya scenes and image caches, FBX, glTF, Photoshop, GIMP and Paint.NET images | 2,056 |
| fields no game build ever declared, left by converters and fixers | 281 |
| Windows executables or DLLs inside the WAD | 66 |
| other archives (zip, RAR, Zstandard) inside the WAD | 40 |
| the game's own skins shipped beside the mod's, up to 68 of them in one mod | 31 |
| shortcuts and `desktop.ini` | 32 |

The skin bin is routinely copied to every slot of the champion, 99 times, so that the mod
shows up whichever skin the player picks: 54,893 such copies in a first corpus of 1,029 mods.
Effects textures are borrowed from other skins. Game files are shipped as they were years ago,
which against the current install look like the author's own work and would override the
game's newer ones. Each of these is recorded in [docs/mod-artifact/](docs/mod-artifact/) with
how it is recognised, why it is there and what the repair is.

## Compared with other fixers

| | **reforge** | Hematite | Topaz / cslol-go | LtMAO | Mod-Repather |
|:--|:--:|:--:|:--:|:--:|:--:|
| **Knows every past patch** | ✅ | ❌ | ⚠️ | ❌ | ❌ |
| **Dates the mod** | ✅ | ❌ | ⚠️ | ❌ | ❌ |
| **Tells stale game files from the author's** | ✅ | ❌ | ❌ | ❌ | ❌ |
| **Field types from a schema** | ✅ | ❌ | ❌ | ❌ | ❌ |
| **Riot's own conversions** | ✅ | ⚠️ | ⚠️ | ❌ | ❌ |
| **Ports to the right skin** | ✅ | ❌ | ❌ | ❌ | ❌ |
| **Voice and HUD as toggles** | ✅ | ❌ | ❌ | ❌ | ❌ |
| **Safe by construction** | ✅ | ❌ | ❌ | ❌ | ❌ |
| **Never extracts to disk** | ✅ | ❌ | ❌ | ❌ | ❌ |
| **Every change recorded** | ✅ | ⚠️ | ⚠️ | ⚠️ | ⚠️ |
| **New patch without a maintainer** | ✅ | ❌ | ❌ | ❌ | ❌ |
| **Rules are data** | ✅ | ✅ | ⚠️ | ❌ | ❌ |
| **.modpkg output** | ✅ | ⚠️ | ❌ | ❌ | ❌ |
| **Platform** | Windows, Linux | Windows | Windows | Windows | Windows |
| **One-time download** | ~1.7 GB | small | small | small | ~220 MB |
| **Maturity** | released, 0.1.4 | released | released | released | released |

The rows come from the other tools' sources and READMEs, read on 2026-10-06, not from running
them. The names are those of the tools; nothing here is a claim about their authors.

The difference is the history. The other tools see the mod against one build, the current
one, and are told by hand what changed in between. reforge sees the mod against every build
the game shipped, so it can say which build the mod was made at, which of its files are the
game's own of that build, which objects are the author's, and what Riot did to each field
between that build and this one. The same history lets a port put a mod on the one skin it
was made for instead of every slot.

The cost is that history: the first run downloads 1.73 GB and keeps 4.77 GB on disk
([ADR-0078](docs/adr/0078-the-history-index-holds-every-entrys-object-bytes-so-a-run-needs-no-clone.md)).
The other tools in the table run with a few hundred megabytes of hash tables or less. reforge
is also younger than all of them.

## What a run needs

| input | where it comes from |
|---|---|
| the game install | the target build's own bins, WADs and hash lists |
| the hash tables | the cache the League Toolkit programs share, downloaded by the app when asked |
| the schema database | the types the target build reads for each field of each class, from rito-meta |
| the history index | every file and object of every build since the census began, as a SQLite database |

The history index is the part that costs something: the first run downloads 1.73 GB once and
keeps 4.77 GB on disk. The app keeps it current by appending builds as they arrive.

## Build

```
cargo build --workspace --release
cargo test --workspace
```

The app's front end lives in [app/](app/) and is built with `pnpm` (`pnpm check` for the
checks). A release is cross-compiled for Windows on Linux.

## Docs

| folder | holds |
|---|---|
| [docs/design/](docs/design/) | the architecture, the history, the roadmap and the app's roadmap, the diagnostic codes, the edge cases, the runs log, the review of ltk-forge |
| [docs/adr/](docs/adr/) | one decision per file: a choice that could have gone another way, and why it went this way |
| [docs/game-change/](docs/game-change/) | one change Riot made per file: the build, before and after, Riot's own conversion |
| [docs/game-behavior/](docs/game-behavior/) | how the client behaves with a file, a value or a name |
| [docs/mod-artifact/](docs/mod-artifact/) | what tools and authors put in mods, how to recognise it, the repair |

[CONTEXT.md](CONTEXT.md) is the glossary: one name for one thing, used everywhere in the repo.
[AGENTS.md](AGENTS.md) is the guidance for anyone, person or program, changing the code.

## License

GPL-3.0-or-later. See [LICENSE](LICENSE).

## 🤖 Disclaimer: Robots Inside

Much of this repository was written with an AI coding agent (Claude, by Anthropic) working
under a person's direction: the code, the tests, the records in `docs/` and this README. The
person made the decisions, which are the ADRs with their dates, checked the output in game, and
reviewed what landed. The agent wrote what the decisions asked for, ran the corpus, and kept the
docs in step with the code.

That is not the same as asking for a mod fixer and taking what came out. Most of the time went
into the prompting and the supervision, not the typing: reading each run's reports, arguing
over each rule, sending work back, and deciding what the evidence meant. What is here is what
survived that:

- **It was built three times.** The work started on 2026-09-19 as ltk-ah-fix, a tool for one
  kind of mod: custom skins whose `skin0.bin` was a pasted copy of a premium skin's. Finding
  those meant comparing meshes, skeletons and clips against the game's, and that grew into
  dating mods and reading the census history, until the one-case tool was the wrong shape and
  was deleted. ltk-forge, the second program, was the proof of concept over the whole corpus:
  it ported 5,136 of the 5,137 mods and was then retired, because it had grown by accretion and
  nobody could say which of its rules were load-bearing. reforge is the rewrite, compared with
  forge stage by stage over every mod until it matched, and then compared with its own
  baseline from there
  ([ADR-0044](docs/adr/0044-ltk-forge-is-retired-and-reforge-is-compared-with-its-own-baseline.md)).
  Three programs and 28 working sessions in under three weeks, each session a person reading
  what came back and sending most of it back.
- **Every rule was checked against the data, not asserted.** The corpus is 4,909 champion skin
  mods of about 6,000 archives, run whole more than ten times between the two programs, with
  668 edge cases counted
  ([CASES.md](docs/design/CASES.md)) and a test for each. A field's type at a build is read off
  the game's own schema or the census history of 757 builds and about 11 million objects,
  never guessed. Riot's conversions are read off Riot's own files across the build that
  changed them.
- **The reasons are written down.** 88 decisions, 25 game changes, 27 game behaviors and 38
  mod artifacts, each a record with its evidence, and a runs log of what was measured.
- **The output was checked in the game**, by a person, on a running client, and the crashes
  found there became fixtures and tests before they became fixes.

Nothing in a mod is ever handed to an AI model: a run is ordinary deterministic code, offline,
with no model in the loop. The agent is a tool of the development, not a part of the program.
