# Review of ltk-forge

What ltk-forge is at commit `d6cd66f` (2026-10-03), read for the rewrite: what works and stays,
and what the rewrite changes. Paths are relative to the forge repository. The design that
answers this review is in [ARCHITECTURE.md](ARCHITECTURE.md), the order of work in
[ROADMAP.md](ROADMAP.md).

forge is 23,900 lines of Rust in seven crates, 14,000 of them in `forge-passes`, and 3,400 lines
of design docs. Every source file and doc was read. Nothing was run.

Faults of forge's code are listed here only where they bind the new design. They are not fixed
in forge.

## What works and stays

These are forge's results. The rewrite keeps each one.

- **Facts apart from policy.** The analyses say what is true of a mod. The policy chooses the
  target slot from them. S, O and T are the vocabulary for it.
- **Every stage's answer prints.** The textual dump shows a wrong decision before anything is
  written. Tests print it on failure in 95 places.
- **Deterministic output.** Typed ids, arenas filled in a fixed order, ordered maps.
- **The rules for untrusted input.** Caps, deflate and stored only, entry names as hashes,
  decoders under `catch_unwind`, text cleaned before a terminal.
- **Diagnostics with stable codes and channels.** The corpus tooling is built on the codes.
- **The history as a derived index.** Exact keys, a stamp, disposable, appended to.
- **Repairs read off Riot's own conversion.** A rule is a table with its evidence, not a guess.
  The contract is idempotence and all or nothing per property.
- **The output form.** A PTCH over the game's bin through a `game_data` declaration, the mod's
  own objects and files under a namespace, voice and HUD in layers.
- **Verification as the overlay applies it.** The port's manifest is loaded and applied to the
  game's bin, and what the patched entry reaches is resolved.
- **Development against a corpus.** A reduced corpus as the regression suite, full datasets for
  a baseline, sample mods checked in game.
- **The leaf crates.** `forge-magic` and `forge-wwise` have natural boundaries.

## The compiler shape, taken literally

forge copies LLVM's pass manager. The mechanisms cost more than they return here.

- **A registry of strings.** 44 pipeline entries carry a name, a kind, a milestone, a design
  reference and `after` edges (`forge-passes/src/pipeline.rs:98`). A check at every run
  validates the order. The order is fixed in practice: three named pipelines select from one
  list. The `after` chain of the emitter repairs records the registry order, so a real
  dependency and an accidental one look the same.
- **A roadmap in the code.** Five pipeline entries and two analyses are stubs with a milestone
  number. The runner has rules for a stub that a built pass depends on.
- **An analysis cache with invalidation.** Analyses are keyed by `TypeId`, record their
  dependencies while they run, and are dropped by a `Preserved` set
  (`forge-passes/src/manager.rs`). In a `port` run 12 of 15 analyses run once and 3 can run
  twice. The transitive drop never changes an outcome. `Preserved` means "keep the snapshot",
  not "still true": `relocate` renames objects in place and returns `Preserved::All`, and
  `verify-apply` then reads `reach` as computed before the rename. The results are right only
  while nothing recomputes them.
- **Analyses that are not analyses of the mod.** `field-types` reads the schema and the game,
  never the module. `inventory` is asked for by no pass. `reach` and `file-refs` depend on the
  target and are the first half of the port plan.
- **A module-wide cache for per-unit questions.** The driver asks for the facts of a slot
  outside the cached candidates, so `slot_facts` and `clips_at` are public and a result keeps
  hidden state for them.
- **Unit structs as keys.** Each analysis has three names (the `XPass` struct, its output, its
  `NAME`) and a fourth in the registry.
- **Borrowed words.** IR, module, lowering, driver, linker, internal linkage. They name the
  mechanism, not the domain, and a reader has to translate them back.

## One mutable module

`Module` (`forge-ir/src/module.rs:463`) holds the archive's facts, marks from several passes,
the decision, and the output.

- Marks on input nodes: `FileNode.used`, `dest`, `edited`, `from`, `taken`; `ObjectNode.edited`,
  `scope`; `SkinUnit.target`, `state`.
- The output (`Module.output`) points back into the input arenas by `FileId` and `ObjectId`.
  The port's objects are the input's objects, renamed in place.
- No type says which stage a module is in. Every pass after `drop-copies` filters
  `UnitState::Live` by hand. One pass checks `target.is_some()` instead.
- The output types live in `forge-ir`. The manifest is not stored: it is rendered as text at
  four sites.
- Layers, WAD names, dropped-unit reasons and check names are strings.

## Stage boundaries

The user's split is analysis, fixups, restructuring, and lowering and writing. forge crosses
each boundary.

| what | where | crosses |
|---|---|---|
| `scope` marks the objects the port may write | `transform/scope.rs` | an analysis stored in the IR, registered as a frontend pass, run as a transform; a second reachability walk that `relocate` checks against `reach` |
| `synthesize` takes the game's bins for a binless mod | `frontend.rs:554` | the frontend calls an analysis (`rework`) and two repairs (`legacy`, `moved`) |
| `stale`, the WAD's champion, duplicate WADs | `frontend.rs` | judging in a file whose header says nothing here judges |
| which units leave the port | `transform/retarget.rs:61` | a decision inside a transform, from seven analyses; not printed, not reported |
| `origin`, `skin-copies`, `rework` | `analysis/` | evidence and verdict in one result |
| `decide` | `driver.rs:215` | computes missing facts, chooses, writes 16 diagnostics, then rewrites its own decisions; runs four times a run, three into a discarded sink |
| `audio-lower` | `transform/audio.rs:122` | restructuring, lowering and bank serialisation in one 218-line function |
| `root-bin-edits` | `hud.rs:64` | analysis, placement, lowering and a layer declaration |
| `file-refs` | `analysis/refs.rs` | an analysis that calls transforms, lowering and the frontend, and learns names |
| verify | `verify.rs` | reports errors that do not stop the write |

The layering runs both ways inside `forge-passes`: analyses call transforms and lowering, and
the frontend calls analyses.

## Code shape

- **Free functions.** Every pass is `fn(&mut Module, &Env, &mut AnalysisManager, &mut DiagSink)`.
  Helpers thread the same four values, and more: `slot_facts` takes seven parameters,
  `unread_at_source` six.
- **Long functions.** Twelve are over 140 lines: `lower::run` 231, `verify::apply` 229,
  `audio::run` 218, `ReachPass::run` 195, `decide` 187, `RigPass::run` 178, `relocate::run`
  174, `index::write` 170, `survey` 151, `open_bytes` 150, `synthesize` 148, `OriginPass::run`
  145.
- **Strings and tuples for data.** `UnitState::Dropped(&'static str)`, `Fit::Fits(String)`,
  `RigCheck.aspect: &'static str`, `Result<_, String>` matched by `e.contains("budget")`,
  `Vec<(String, String, Option<String>, Vec<u8>)>`, characters keyed by name though `CharId`
  exists.
- **No view types.** The skin entry's fields (resolver, graph, mesh, skeleton, texture) are read
  field by field at ten sites. The game's skin bin at T is read and parsed at six.
- **No layout type.** The output's path and name formats are spread over six files, with five
  name sanitisers that differ.
- **Duplicated walks.** `forge-ir/src/value.rs` holds five recursive matches over one enum.
  Ten repairs repeat one loop around `structs_mut`. The walk yields a class and its properties,
  with no object or path, so the change records the design asks for cannot be made.
- **No rustfmt.** 56 lines are over 200 columns, the longest 289.

## Fixups

The repair family is 18 passes.

- Three rule shapes are data: remove a field, rename a field, move fields into a child struct.
  They are eight passes and about 60 table rows. Each has its own copy of the frame: is the
  target's shape the new one, did the mod's build read the old field, rewrite, learn names,
  report.
- Four are bespoke code on the same frame (`meshalign`, `spawnshape`, `surface`, `strays`).
- Three conform values to the schema with no table (`reshape`, `retype`, `strip-undeclared`).
- Two file repairs run before the mod is structured (`legacy-formats`, `moved-assets`), and one
  rewrites a bank's header.
- A game change and a tool artifact share a pass in two places: `reshape` (wrap and hoist) and
  `vfx-spawn-shape`.
- Order matters and is implicit: strays before legacy-simple, the gathers before reshape,
  everything before retype, known artifacts before strip-undeclared.
- The evidence for a rule is in its module comment and again in a design doc.
- The repairs touch only objects `scope` marked. Without `scope`, a whole-WAD repack's gameplay
  data counted as 18,842 lost fields against 1,117.

## Diagnostics

- The design says the report aggregates records, never sentences. Most diagnostics are a
  sentence with the counts inside it. One pass in the transforms carries a structured payload.
- Payloads are `serde_json::Value` built by hand. All 15 analysis dumps build their JSON by hand.
- "The first N and M more" is rebuilt at six sites.
- Every code is declared in `forge-ir`, so the lowest crate names every pass.
- A subject is a string, not an id.
- An analysis keeps its diagnostics in the cache. A halted run loses those of a dropped analysis.

## Names

- `Env` is documented as unchanged during a run. `Names` sits in it and is written through
  `&Env` by about 25 `learn_*` calls, from analyses too.
- `Module.made_names` is a second store, written beside the first.
- The source order differs by lookup: paths are learnt first, entries and fields learnt last.
- One lookup collects learnt paths in hash-map order, so a collision's winner varies by run.
- The hex fallback is written inline at 14 sites.

## The game side

- `GameSource` mixes a chunk store, WAD membership and one analysis's size index. It has no
  content id and no parsed bin, so callers repeat read, hash and parse, and each swallows the
  read error.
- Nothing is cached. One mutex covers every read of the install.
- `MemoryGame`, the test fake, lives in the production crate.

## History

- The query side is one type with typed methods, `HistoryIndex`. No pass writes SQL.
- The crate is documented as a leaf that knows neither the IR nor `ltk_*`. It depends on
  `forge-ir`, `forge-wwise`, `ltk_meta` and `ltk_ritobin`, all for one function that reads what
  an entry text names. The workspace patches five `ltk_*` crates for it.
- 13 of 18 tables have no reader in any pass. They hold most of the rows: 8.4 million refs,
  6.5 million event wems, 1.6 million textures.
- A family's fields are spelled in about seven places between the walk, the write and the
  append.
- Tests need a real SQLite file in a temporary directory. Seven test files build one by hand.

## Untrusted input

The rewrite's types have to rule these out.

- An archive entry's path is kept as spelled and reaches a filesystem path in the in-place
  output. A path that names a place on disk is a different type from a path spelled by a mod.
- The bank reader copies one slice per index entry with no total budget.
- `bins::parse`, the packed WAD mount, the `.sco` reader and the bank reader run outside
  `catch_unwind`.
- The archive's bytes are held up to three times for the whole run.
- The emit step deletes the existing project before it knows the port holds anything.

## Tests and corpus tooling

- 137 tests in `forge-passes`. Passes are reached by registry names
  (`parse_passes("open,survey,load,...")`), and assertions go against module fields, code
  strings and message substrings.
- The `Env` literal is written out in 16 test files. Eight local helpers format schema JSON by
  hand.
- Lowering, emit and verify are tested end to end only. One test checks determinism, on the
  dump of one trivial mod. Nothing compares written bytes across two runs.
- The corpus tooling is not in git: `port_run.py`, `diff.py`, `reduce.py`, `summary.py` and 14
  more scripts, the reduced corpus lists, the baseline runs. They hard-code local paths.
- `diff.py` compares unit ids, states, decision reasons and diagnostic codes as raw strings.
  A rewrite that renames any of them needs a mapping to compare itself with forge.

## Docs

- Facts about the game and about tools are spread over design docs, module comments and the
  runs log. A field's history is found by reading `TRANSFORMS.md`, the pass's header and
  `RUNS.md`.
- Design docs carry status logs and dated build notes (`AUDIO.md`, `WRITER.md`,
  `TRANSFORMS.md`).
- Some statements are stale: `WRITER.md` gives a layout the code replaced, and a conversion
  (`null-pointer`) and per-build retyping are documented and not built.
- The user's decisions are dated notes inside paragraphs. There are about 25.

## Smaller things

- `own-resolver` failed in game and is opt-in, and its branches stay in lowering and the output
  types.
- `FileKind::keeps_bytes` always returns true, which leaves a dead branch in `load`.
- `Options.reworks` is a test hook in the options struct. The policy enums sit in `env.rs`.
- A new sub-character bin gets no override record, so verify skips it.
- `target.conflict` looks unreachable: its rank ends with the source slot.
- A copied submesh that is hidden and never shown is reported as "fits with a fix". No pass
  removes it.
