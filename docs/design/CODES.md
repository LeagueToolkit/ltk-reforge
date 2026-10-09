# Diagnostic codes

A code is permanent from the time the module that raises it lands (#ADR-0008). `reforge
diagnostics` lists the codes that are declared. A code of this table that it does not list may
still change.

The scheme is in [DIAGNOSTICS.md](DIAGNOSTICS.md), "Names". The last column is ltk-forge's code
for the same finding. It is the map for a person who reads a report of ltk-forge's, and no
program reads it (#ADR-0044). "none" marks a finding ltk-forge does not report.

Channels: `input` is the archive as shipped, `decision` the facts and the choice of target,
`output` what the package holds, changes or loses, `run` the run's other inputs.

## archive

The zip and its entries. Channel `input`.

| code | severity | means | ltk-forge |
|---|---|---|---|
| `archive.not-zip` | fatal | the archive is not a zip, or does not read | `archive.not-zip` |
| `archive.too-many-entries` | fatal | more entries than the cap | `archive.too-many-entries` |
| `archive.budget-exceeded` | fatal | more inflated bytes than the cap | `archive.budget-exceeded` |
| `archive.no-skin` | fatal | no WAD ships a champion skin bin or a file at a skin's path: not a skin mod | `archive.no-skin` |
| `archive.entry-too-large` | error | an entry over the cap for one entry, skipped | `archive.entry-too-large` |
| `archive.entry-corrupt` | error | a compressed entry fails its checksum, skipped | `archive.bad-compressed-crc` |
| `archive.entry-unreadable` | error | an entry that does not open or read, skipped | filed under the two codes beside it |
| `archive.codec-unsupported` | error | an entry in a codec other than deflate or stored, skipped | `archive.unsupported-compression` |
| `archive.name-unsafe` | error | an entry whose name is no path inside a WAD (a `..`, a root, a drive), skipped | none |
| `archive.wad-unreadable` | error | a packed WAD does not mount | `archive.packed-wad-unreadable` |
| `archive.stored-crc-wrong` | note | a stored entry with a wrong checksum, read as stored | `archive.bad-stored-crc` |
| `archive.repack-duplicate` | note | a chunk with a numeric suffix a repack kept, skipped | `archive.repack-duplicate` |
| `archive.chunk-duplicate` | note | a second chunk at one path, skipped | `archive.duplicate-chunk` |
| `archive.info-unreadable` | note | `META/info.json` does not parse | `archive.info-unreadable` |

## inventory

What the archive carries besides game files. Channel `input`.

| code | severity | means | ltk-forge |
|---|---|---|---|
| `inventory.executable` | warning | programs in the archive, never run or examined | `inventory.executable` |
| `inventory.bin-text` | warning | a bin as text or JSON where the client reads a binary bin | `inventory.bin-text` |
| `inventory.source` | note | authoring files: scenes, layered images, bins as text | `inventory.source` |
| `inventory.nested-archive` | note | an archive inside the mod, not opened | `inventory.archive` |
| `inventory.os-artifact` | note | leftovers of file browsers | `inventory.os-artifact` |
| `inventory.unknown` | note | files of no known kind | `inventory.unknown` |

## wad, character

A WAD of the mod and the characters in it. Channel `input`.

| code | severity | means | ltk-forge |
|---|---|---|---|
| `wad.locale` | note | a locale WAD; its voice goes to a voice layer | `survey.locale-wad` |
| `wad.locale-packed` | note | a locale WAD holding the whole mod, taken as the champion's | `survey.locale-packed` |
| `wad.champion-guessed` | note | a WAD named for no champion, taken as the one champion whose skin bins it ships | `survey.wad-guessed` |
| `wad.champion-ambiguous` | warning | a WAD named for no champion ships several champions' skin bins; taken as none | `survey.wad-ambiguous` |
| `wad.duplicate` | note | a second WAD for one champion; the one the game spells is carried | `survey.duplicate-wad` as a note |
| `wad.duplicate-differs` | warning | the second WAD holds files the carried one lacks | `survey.duplicate-wad` as a warning |
| `wad.several-champions` | note | more than one champion WAD | `survey.multiple-champions` |
| `wad.not-carried` | note | a WAD with nothing of a champion's skin | `survey.other-wad` |
| `character.equal-to-game` | note | a sub-character whose skin bins are the game's, left to the game | `survey.sub-equal-to-game` |

## file

A file of the mod. Channel `input`, but the four a file fixup raises, which are `output`.

| code | severity | means | ltk-forge |
|---|---|---|---|
| `file.unused` | note | files nothing loads; not judged or carried | `load.unused` |
| `file.stale` | note | files holding the bytes the game shipped at their paths at earlier builds; not carried | `load.stale` |
| `file.former-path` | note | files at paths the game shipped at earlier builds and does not ship | `load.former-game-path` |
| `file.older-game` | warning | files that are bytes the game shipped at earlier builds, which the install does not hold | `files.older-game` |
| `file.same-size-capped` | note | files that meet more game chunks of their size than the cap; the rest are not read for their bytes | none |
| `file.format-converted` | note | files in a format the game dropped, made the one it reads at the same stem | `legacy-formats.converted` |
| `file.format-unreadable` | warning | such files that do not read, left where they are | `legacy-formats.failed` |
| `file.moved` | note | files at former paths, made where the game ships the asset | `moved-assets.followed` |
| `file.move-failed` | warning | such files that do not convert, left where they are | `moved-assets.failed` |

## bin, rework

A bin of the mod, and what a champion's rework says of the mod. Channel `input`.

| code | severity | means | ltk-forge |
|---|---|---|---|
| `bin.unreadable` | warning | a loaded bin that does not parse | `load.bin-unparsable` |
| `bin.entry-duplicate` | warning | a bin lists one entry hash more than once; the first is kept | `load.duplicate-entry` |
| `bin.link-missing` | note | linked paths neither the mod nor the game holds; the client skips them | `load.missing-link` |
| `bin.load-capped` | note | a root's load reached the cap on bins; the mod's bins linked past it are not loaded from it | none |
| `bin.taken-from-game` | note | a mod with no skin bin: the game's bins naming the files it overrides are taken as its own | `synthesize.game-bins` |
| `rework.restoration` | note | a restoration of a champion's model from before its rework; its copies of older game bytes are its own | `synthesize.restoration` |

## fixup

What the fixups did. Channel `output`. The fixup is in `by`.

| code | severity | means | ltk-forge |
|---|---|---|---|
| `fixup.changed` | note | values kept, in the form the target build reads | `retype.converted`, `migrate.moved`, `reshape.wrapped`, `reshape.hoisted`, every `vfx-*.moved` |
| `fixup.removed` | note | values removed with nothing lost: never read by a build since the mod's own, or written by a tool and read by none | `retype.dead-at-source`, `strip-undeclared.removed`, `strip-known-artifacts.removed`, `material-default-technique.removed` |
| `fixup.lost` | warning | values removed that the mod's own build read and the target cannot hold | `strip-undeclared.lost`, `material-default-technique.other` |
| `fixup.unconvertible` | warning | values with no conversion, left as they are; the client skips them | `retype.unconvertible` |

## catalog, copies, origin, rig, target

The facts and the decision. `catalog` and `copies` are channel `input`, the rest `decision`.

| code | severity | means | ltk-forge |
|---|---|---|---|
| `catalog.capped` | note | the game's skins of a character whose bins link more than the cap read of one skin; the rest are not read | none |
| `copies.found` | note | the copies of a representative unit, with their slots | `copies.dropped` |
| `copies.near` | note | a near copy: another build of the same work | `copies.near` |
| `copies.distinct-units` | note | several distinct skin bins in one character | `copies.distinct-units` |
| `origin.ambiguous` | warning | premium skins tie on evidence | `origin.ambiguous` |
| `origin.conflicting` | note | strong evidence for both the base and a premium skin | `origin.conflicting` |
| `origin.none` | note | no evidence decides | `origin.none` |
| `rig.submesh-copied` | warning | a submesh copied whole from a premium skin, with its visibility | `rig.copied-submesh` |
| `rig.unreadable` | warning | a mesh, skeleton or clip that does not parse | `rig.unreadable` |
| `target.chosen` | note | the unit's target and the reason: base, paste cleaned, slot, fallback, clips origin, skeleton origin, follows its champion | `target.base`, `target.paste-cleaned`, `target.slot`, `target.fallback`, `target.clips-origin`, `target.sub-follows` |
| `target.forced-unfit` | warning | a target a person or the champion forced, which the work does not fit | `target.forced` |
| `target.superseded` | note | another unit of the character took the slot | `target.superseded` |
| `target.conflict` | warning | units of one character, of the same rank, target one slot; each leaves it to the game | `target.conflict` |
| `target.sub-left` | warning | a sub-character whose model does not fit the base, left to the game | `target.sub-left-to-base` |
| `target.unjudged` | warning | no rig facts; the unit stays at its slot unjudged | `target.unjudged` |
| `target.infeasible` | error | the work fits no slot; written at skin0 for a person to decide | `target.infeasible` |
| `target.pre-rework` | error | made for the champion's model before its rework; left to the game | `target.pre-rework` |

## unit, object, ref

The port's plan and each unit's part of the package. Channel `output`, but
`object.unreached`, which is `input`. `unit.new-bin` is raised by the build, which writes the
bin. `unit.left` is raised once per character, with each of its units that leaves.

| code | severity | means | ltk-forge |
|---|---|---|---|
| `unit.left` | note | the units of a character that leave the port, and why: a copy, superseded, tied with another unit on its target, made before a rework, a sub-character left to the base or equal to the game's, no target, the game's own skin, a chroma folded into its parent, at a slot the port does not change | `drop-copies.dropped` |
| `unit.left-with-work` | warning | own work that leaves with a character's units at slots the port does not change, which the units at the target do not hold: models, skeletons and textures as the author's work for that slot, objects, other files and clips alone as not supported for now | none |
| `unit.retargeted` | note | the unit's names moved from its slot to its target | `retarget.renamed` |
| `unit.new-bin` | note | a sub-character the game ships only at skin0 gets a skin bin at the target | `target.sub-no-slot-bin`, `lower.new-bin` as a note |
| `unit.new-bin-failed` | error | a record did not apply to that new bin | `lower.new-bin` as an error |
| `unit.not-written` | error | the game has no bin to patch at the target, or it does not read | `lower.no-game-bin` |
| `unit.ported` | note | what the unit's port holds: records, objects carried, whether the graph ships, whether its bin is written whole | `lower.summary` |
| `object.carried` | note | what the port carries under its namespace: objects, files, references pointed at the game's files | `relocate.summary` |
| `object.game-carried` | note | game objects carried because they name files the mod overrides | `reach.overrides` |
| `object.link-dangling` | note | names the entry reaches that nothing loaded at the target holds, nor the game's bins of another skin of the character | `reach.dangling` |
| `object.copied` | note | game objects of other skins copied under the target's names, for names nothing loaded at the target holds (#ADR-0083) | none |
| `object.unreached` | note | objects of the unit's load that no entry reaches; not carried | `reach.leftovers` |
| `ref.dangling` | note | files the port names that nothing holds | `file-refs.dangling` |
| `ref.unmounted` | note | files only a WAD not mounted with the champion holds | `file-refs.unavailable` |

## audio, hud

Channel `output`.

| code | severity | means | ltk-forge |
|---|---|---|---|
| `audio.carried` | note | a unit's own clips carried, in bank units after the game's: each bank with its unit, its layer, how its clips play, the game's bank its events are cut from in place of one the client no longer loads, and the game unit it stands in for | `audio.carried`, once for each bank, layer and stand-in |
| `audio.not-carried` | warning | a unit's own clips not carried, each with why: no audio block at the target, a locale copy that differs, no events bank to cut, no event that plays them | `audio.not-carried` |
| `audio.units-left-out` | note | the game's bank units at the target the port leaves out of the block, as the mod's own block does, each with the first build the game's skin held it (#ADR-0063) | `audio.units-left-out` |
| `audio.literals-renamed` | note | literal sound names rewritten for the target's audio: to the mod's renamed events, to the base's | `sound-literals.rewritten` |
| `audio.bank-id-set` | note | a bank whose header id was zero, given its name's hash | `bank-id.set` |
| `audio.bank-unnamed` | warning | a bank with header id zero whose path no table spells | `bank-id.unnamed` |
| `audio.bank-unreadable` | warning | a bank that does not read; its clips are not judged | none |
| `hud.carried` | note | spell icons and HUD textures shipped in the HUD layer | `root-bin-edits.carried` |

## package, check, write

What was built, what the checks found, what was written. Channel `output`.

| code | severity | means | ltk-forge |
|---|---|---|---|
| `package.field-kept` | note | a carried field the mod's entry lacks; the game's stays | `lower.field-kept` |
| `package.entry-whole` | warning | the game's bin has no entry under the target's name; the mod's goes in whole | `lower.entry-whole` |
| `package.resolver-frozen` | note | the resolver's map is written whole; keys the game adds later are lost until a rebuild | `write.resolver-frozen` |
| `package.texture-contained` | note | DDS textures put in TEX's container; those kept as DDS, and why | `tex-container.swapped` |
| `package.in-place` | note | a repair's output: files kept, bins written again, and the files left out because the client does not reach them, by reason: nothing loads or names them, at a former path, a stale copy, named only by removed values | `inplace.summary` |
| `package.ported` | note | a port's output: files shipped, bins made, patches, and the files left out because the client does not reach them through the port, by reason | none |
| `package.empty` | fatal | the package holds nothing | `emit.empty` |
| `package.shader-missing` | fatal | materials linking a shader the target's shader bin lacks, with no relink: the client crashes at load (#ADR-0069) | none |
| `package.thumbnail-dropped` | note | the mod's image is not carried as the thumbnail, and why (#ADR-0051) | none |
| `check.manifest-unreadable` | error | the package's manifest does not load | `verify.manifest` |
| `check.apply-failed` | error | a patch does not apply to the game's bin | `verify.apply` |
| `check.record-skipped` | error | records the overlay would skip | `verify.record-skipped` |
| `check.link-unresolved` | error | links the package adds that resolve to nothing | `verify.link` |
| `check.load-capped` | note | a patched bin whose load reached the overlay's cap on bins; names past it are not checked | none |
| `check.name-unresolved` | error | names the patched entry reaches that resolve to nothing, and did resolve in the mod | `verify.unresolved` |
| `check.file-missing` | error | files the package names that are neither shipped nor the game's | `verify.file` |
| `check.other-skin` | warning | values of a champion's patched base skin that name another skin of the champion (#ADR-0062) | |
| `check.type-mismatch` | error | values written of another type than the target build declares | `verify.type` |
| `check.field-undeclared` | error | fields written that the target build's class does not declare | `verify.undeclared` |
| `check.round-trip-failed` | error | a rewritten bin does not read back as it was built | `inplace.verify` |
| `check.done` | note | what each check covered | `verify.summary` |
| `write.written` | note | the project directory and the package | `emit.written` |
| `write.blocked` | error | the caller's policy refused the write, and for which diagnostics | none |

## install, names, history, schema

The run's other inputs. Channel `run`.

| code | severity | means | ltk-forge |
|---|---|---|---|
| `install.wad-unreadable` | warning | a WAD of the install whose table of contents does not read | `install.wad-unreadable` |
| `install.version-unknown` | warning | the install does not say its build | `install.version-unknown` |
| `names.mimir-absent` | note | the hash cache was not found | `names.no-mimir` |
| `history.absent` | note | no history: judged against the install only | `history.absent` |
| `history.index-missing` | error | a history was given and its index does not open | `history.index-missing` |
| `history.index-stale` | warning | the index is of another commit than the history's head | `history.index-stale` |
| `history.install-newer` | warning | the install is newer than the history's newest build | `history.install-newer` |
| `schema.absent` | note | no meta schema database; types come from the install's bins | `schema.absent` |
| `schema.unreadable` | warning | the meta schema database does not read | `schema.unreadable` |
| `schema.install-newer` | warning | the install is past what the schema describes | `schema.install-newer` |

## ltk-forge's codes with none here

| ltk-forge | why |
|---|---|
| `scope.outside`, `scope.unrepaired` | every loaded object is fixed (#ADR-0002) |
| `own-resolver.written` | not carried over (#ADR-0006) |
| `pass.unimplemented` | no stub passes (#ADR-0006) |
| `emit.skipped` | writing is the caller's call |
