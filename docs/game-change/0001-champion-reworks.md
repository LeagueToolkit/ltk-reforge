# GAME-CHANGE-0001: Champion reworks

- **Build:** one build per champion, 13 from 10.11 to 16.6 (below)
- **Affects:** a champion's base mesh and skeleton, every skin's mesh, skeleton, clips and
  effects, and the paths under the champion's folders
- **Related:** none

## Before

A champion's model as it shipped until the build of its rework: a base skeleton with its joint
names, a base mesh with its submeshes, and every skin built on them.

## After

A new model for every skin at once: another skeleton with other joint names, other submeshes,
new clips and effects. From 19% to 100% of the paths under the base folder are dropped at that
build.

| champion | build |
|---|---|
| Volibear | 10.11 |
| Dr. Mundo | 11.12 |
| Caitlyn | 11.23 |
| Udyr | 12.16 |
| Ahri | 13.3 |
| Jax | 13.20 |
| Skarner | 14.7 |
| Lee Sin | 14.9 |
| Teemo | 14.20 |
| Viktor | 14.24 |
| LeBlanc | 15.7 |
| Xin Zhao | 15.17 |
| Shyvana | 16.6 |

A re-export is no rework. It may rename every submesh, but the joint count stays within two and
at most 2% of the paths are dropped: Vi 15.14, Taliyah 15.17, Zac 15.22, Tahm Kench 15.24, Quinn
16.4. A re-encode or a format upgrade changes neither the joint names nor the submeshes.

## Riot's conversion

None. Every skin is rebuilt on the new model. An old path has no successor: the file at the new
path of the same role is another asset.

## In a mod

A mod made before the rework has textures laid out for the old mesh and a bin naming the old
files. On a later build its textures sit on the wrong mesh, or its files are at paths nothing
loads.

A mod that restores the old model is made after the rework. It ships the old mesh, skeleton and
most of the clips it plays as its own files, and most such mods ship a path of the new model
too.

## Evidence

- Found on the census history by the base skeleton's joint names and every base submesh
  replaced in one build, with the share of the base folder's paths dropped there. The history
  covers builds from 8.20.
- A change of a mesh's or a skeleton's bytes alone does not count: re-encodes and format
  upgrades change bytes and keep names.
- ltk-forge's eighth datasets run (2026-10-01) found 15 of 5,137 mods made before their
  champion's rework.
