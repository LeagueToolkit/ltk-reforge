# MOD-ARTIFACT-0034: Transparent placeholders at missing texture paths

- **Source:** League-Mod-Repather, the repath tool that preceded Hematite
- **Affects:** `.dds` and `.tex` files at paths under a repath prefix (#MOD-ARTIFACT-0029)
- **Related:** #MOD-ARTIFACT-0029, #MOD-ARTIFACT-0033

## What is found

A texture whose bytes are one of two files the tool ships, each 256 by 256 in DXT5, every block
the same: alpha 0, colour white. The texture is fully transparent.

| file | size | SHA-256 | header |
|---|---:|---|---|
| `invis.dds` | 65,664 | `149fb436fa1ed840419ca6a987fb86f171bbb74ac85dcd6b073ed56b6538cd4d` | DDS, with `GIMP-DDS` at offset 32 |
| `invis.tex` | 65,548 | `6a93a907f8b2d8253af1d80379968a65819844db75db7cbbe3812bf82e83065d` | TEX, format 12, no mips |

One copy sits at each path a bin of the mod names that the mod does not hold, with the
extension the bin spells, under the tool's prefix and in lower case. Paths with `/hud/` get none.
A packed WAD holds the copies as one chunk that several entries share.

## Why it is there

The tool puts its prefix into every string of the skin's bins that holds `assets/` or `data/`,
whether or not it holds the file. A string naming a game file the champion's WAD lacks (a
shared particle texture, another champion's file) then names a path that exists nowhere, and the
tool fills each such texture with a transparent one. Meshes, animations and banks so broken are
left dangling.

## What reads it, and how

The client loads the placeholder where the bin names it and draws nothing visible with it: an
effect or a material that used the game's texture shows none.

## The repair

The path with the prefix taken off is the file the author's bin named. Where the game holds a
file at that path, with either extension, the reference goes back to the game's path and the
placeholder is not needed. Where it does not, the reference was dangling before the tool ran.

## Evidence

- The baseline repair run (2026-10-04), over the 5,137 mods of the datasets: 155 mods hold 842
  placeholders. 86 of the mods also hold files under the tool's word prefix
  (#MOD-ARTIFACT-0029), and 55 a bin of #MOD-ARTIFACT-0033.
- 497 of the 842 sit at a path that, its second folder taken off, the game has held, by the
  CommunityDragon game-path tables with either extension (2026-10-06). Most of the rest sit
  under an author's own folder, or under a second prefix a second run of a repath tool added.
- The files, their bytes and where the tool writes them are read from the source of
  League-Mod-Repather (2026-10-05). The two files were hashed, not run. The tool's source also
  names a white placeholder for colour ramps, which its repository does not hold.
