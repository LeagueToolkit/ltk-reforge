# MOD-ARTIFACT-0029: Repath prefixes in asset folders

- **Source:** repath tools: TopazModFixer (the fixer of the cslol-go manager),
  League-Mod-Repather, LtMAO's bumpath and Hematite
- **Affects:** the paths of the files a mod's bins name, and the strings that name them
- **Related:** #MOD-ARTIFACT-0003, #MOD-ARTIFACT-0004, #MOD-ARTIFACT-0026, #MOD-ARTIFACT-0033,
  #MOD-ARTIFACT-0034

## What is found

A file whose path holds, after its root folder, a folder or a part of a folder that no game
path holds. Four shapes are known:

| shape | example | written by |
|---|---|---|
| `.<p><N>_` glued to the second folder, or standing as a folder of its own | `ASSETS/.Yasu54_Characters/Yasuo/Skins/Skin54/...`, `ASSETS/.Ahri0_/characters/...` | TopazModFixer; Hematite's "in folder" layout |
| a word as the second folder | `assets/steel/characters/...`, `ASSETS/fire/Characters/Yone/...` | League-Mod-Repather |
| `bum` as the second folder | `assets/bum/characters/sett/skins/skin38/particles/...` | LtMAO's bumpath |
| `hematite` as the second folder | `assets/hematite/<folder>/...`, `assets/hematite/hematite/...` | Hematite |

- **TopazModFixer's prefix.** `<p>` is the first four characters of the character's name, in
  the case configured, and `<N>` the skin the tool worked on: `.Yasu54_`, `.drmu3_`. A
  sub-character's files carry the champion's prefix. The roots are written `ASSETS` and
  `DATA`.
- **League-Mod-Repather's word.** One of 30 words chosen at random for a run (`fire`, `ice`,
  `storm`, `shadow`, `light`, `dark`, `void`, `star`, `moon`, `blood`, `steel`, `frost`,
  `flame`, `thunder`, `wind`, `dragon`, `blade`, `spirit`, `chaos`, `magic`, `crystal`,
  `poison`, `mystic`, `cosmic`, `royal`, `wild`, `rage`, `fury`, `power`, `death`), or a text
  its user gives. The bin strings keep their case; the files are written in lower case. A
  second run nests a second word and leaves the first.
- **LtMAO's bumpath.** `bum` unless its user gives another text to chosen entries; a new scan
  sets every entry back to `bum`. The prefix goes after the path's first folder, and the files
  are written in lower case while the bin strings keep their case. A string is rewritten only
  where the tool found its file. Its default refuses to run while a named file is missing, so
  its users add the game's unpacked champion WAD as a source, and every file found there is
  copied in.
- **Hematite.** A path with no `assets` or `data` root gets the prefix as a new first folder:
  `hematite/<folder>/...`. A second run on its own output stacks the prefix, and Hematite
  removes a stacked one when it runs again.
- **Collisions.** TopazModFixer and Hematite add `_1`, `_2` before the extension when two paths
  land on one name.
- **Extensions.** TopazModFixer writes a path with the extension of the file it found: a `.tex`
  reference to a `.dds` file becomes a `.dds` path, and a `.sco` becomes a `.scb`.
- **What moves.** TopazModFixer writes its bins at the game's paths and moves textures, meshes,
  animations and banks. Hematite's source moves no `.bin`, `.bnk` or `.wpk` and leaves the
  strings of voice-over paths (`/wwise2016/vo/`) as they are. The mods under `hematite` in the
  datasets hold bins and banks under it all the same, from another version or another tool.
  League-Mod-Repather writes bins at the game's paths, moves banks, and leaves voice-over files
  and `icons2d` at the game's paths. It puts its word into every `assets/` or `data/` string of
  the skin's bins, files it does not hold included (#MOD-ARTIFACT-0034). LtMAO's bumpath moves
  banks too and no bin: a bin it reaches, the game's linked bins included, is written at its own
  path with its strings rewritten (#MOD-ARTIFACT-0004).

It is told from an author's own folder by the shape: a game category (`characters`, `shared`,
`particles`) follows the prefix, glued to it or in the next folder.

## Why it is there

A repath tool moves each file a skin's bins name to a path of the mod's own and rewrites the
strings, so that the mod's files sit beside the game's and replace none. A file shared by
several skins or champions then changes only for the mod's skin.

The tools also copy in every file the bins name that the mod lacks, from the player's install
or from an older game's WADs, and put the copies under the prefix too: TopazModFixer with its
`SmallMod` setting off or its older WADs given, League-Mod-Repather from the player's champion
WAD, LtMAO's bumpath from the unpacked game WAD its user adds, Hematite in its default repair.
Most files under a prefix are such copies.

## What reads it, and how

The client finds a file by the hash of its whole path and reads no meaning into a folder. A
prefixed file loads where a bin of the mod names it.

A copy of a game file under a prefix holds the game's bytes of the build the tool ran at. Riot's
later changes to that file do not reach the mod, as with a stale copy at the game's own path
(#MOD-ARTIFACT-0004), though nothing of the game is replaced.

The copies also date the mod: a file the game shipped at the build the tool ran at puts the
mod's date at that build or later, whatever build the author made it at.

## The repair

None is needed for the files to load.

The path with the prefix taken off, and the extension the bin first spelled, is the path the
tool took the file from. A prefixed file whose bytes the game shipped at that path is the game's
file, not a file the author chose from another path (#MOD-ARTIFACT-0003): the game's own file at
that path stands for it.

## Evidence

- The baseline port run (2026-10-04), over the 5,137 mods of the datasets, by the paths and the
  provenance its reports list:

  | shape | mods | files under it | the game's bytes at the target build | the game's bytes at an earlier build only |
  |---|---:|---:|---:|---:|
  | `.<p><N>_` | 55 | 14,684 | 10,330 | 1,010 |
  | one of the 30 words | 223 | 59,011 | 41,936 | 2,093 |
  | `bum` | 127 | 33,507 | 27,794 | 1,326 |
  | `hematite` | 4 | 1,161 | 1,009 | 4 |

  Textures are most of them: 9,753 `.tex` files under `.<p><N>_` and 19,788 under `bum`. Banks
  and bins: 60 `.bnk` and 16 `.wpk` under `.<p><N>_`, 368 `.bnk` and 72 `.wpk` under `bum`, 47
  `.bin` and 36 `.bnk` under `hematite`.
- The 30 words are about equally frequent among the word-prefixed files (from 3,363 for `steel`
  to 551 for `fire`), as a random choice per run gives. 110 mods carry the text
  League-Mod-Repather writes into `info.json`: a name ending ` (Repathed)`, a description ending
  `Repathed with League Mod Repather.`
- How many of the copies sit at the path their bytes were shipped at, the prefix taken off, is
  not counted.
- Two mods of TopazModFixer's shape whose emitters it rewrote (#MOD-ARTIFACT-0031) are dated
  16.3.7422025 and 16.4.7461248, each by a game file under its prefix
  (`assets/.talo12_characters/talon/skins/skin12/particles/...scb`,
  `assets/.vieg0_characters/viego/skins/skin21/...tex`).
- The shapes and the tools' choices are read from the source of TopazModFixerCLI,
  League-Mod-Repather, LtMAO and Hematite (2026-10-05 and 2026-10-06). Nothing was run.
