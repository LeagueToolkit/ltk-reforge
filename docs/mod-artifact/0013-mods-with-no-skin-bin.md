# MOD-ARTIFACT-0013: Mods with no skin bin

- **Source:** authors' practice: a mod made of files at the game's own paths
- **Affects:** a champion's WAD with no `data/characters/<character>/skins/skin<N>.bin`
- **Related:** #MOD-ARTIFACT-0004, #MOD-ARTIFACT-0010, #MOD-ARTIFACT-0011, #GAME-CHANGE-0001,
  #GAME-CHANGE-0007, #GAME-CHANGE-0008, #GAME-CHANGE-0009, #GAME-CHANGE-0010,
  #GAME-CHANGE-0011, #GAME-CHANGE-0012

## What is found

A WAD named for a champion that holds no skin bin of any slot, and holds files at paths the
game ships: meshes, skeletons, textures, clips, banks, and bins the game shares between skins.
The glossary calls it a binless mod.

A file of such a mod is one of five things:

| the file | means |
|---|---|
| at a path the game ships, with other bytes | an override: the author's work |
| at a path the game ships, with the bytes the game held there at an earlier build | a stale copy (#MOD-ARTIFACT-0004) |
| at a path the game held at earlier builds and does not ship | an override of its day, at a former path |
| at a path the game never held | the author's own file, which nothing names |
| at a path the game ships, with the game's bytes | the game's file again |

It is told from a mod whose skin bins sit in a WAD under another name: a locale WAD
(#MOD-ARTIFACT-0010) or a WAD named for no champion (#MOD-ARTIFACT-0011).

## Why it is there

The client loads a file by its path. A file at a path the game's bins name takes the game's
file's place with no bin of the mod's. An author who changed a texture, a model or a sound
shipped that file alone.

## What reads it, and how

The client loads the mod's file wherever a game object names its path.

- A file shows under every skin whose bins name it. A base skin's texture that a premium skin's
  effect names shows under that skin too.
- A file at a former path is not loaded: no game object names the path. The game moved the
  asset (#GAME-CHANGE-0007 to #GAME-CHANGE-0012) or replaced the model (#GAME-CHANGE-0001).
- A file at a path the game never held is not loaded.
- A bin the game shares between skins, shipped at its path, is loaded through the game's link
  to it.

## The repair

A repair leaves the mod as it is: files at the game's paths are the form of such a mod.

A port puts a unit on a slot, and this mod has none. The game's bins of the skins that name the
overridden files are taken as the mod's own (`bin.taken-from-game`). A WAD with no override
gives no unit, and an archive with no unit at all is no skin mod (`archive.no-skin`).

## Evidence

- The baseline run (2026-10-03): 1,289 of the 5,137 mods, a quarter. Each ports. 3 of them hold
  a second champion WAD with skin bins of its own.
- Those mods hold 27,483 overrides. The game's bins of 2,631 skins are taken for them: of one
  slot in 1,109 mods, and of slot 0 alone in 1,015.
- 3,130 of the overrides are named by no skin's bin, or only by skins other than the one whose
  folder they sit in.
- Beside the overrides those mods hold 4,504 stale copies, 5,446 files at paths the game never
  held, 345 files at former paths with no path found for them, and 5,439 files whose asset has
  another file of the mod.
- ltk-forge's full datasets run (2026-09-30), made before it handled such mods: 1,738 of 5,544
  mods held no skin bin and halted as no skin mod.
