# Glossary

The words this repository uses, one name for one thing. A doc, a comment, a test name or a
commit takes the term here, not a synonym.

## The game

| term | means |
|---|---|
| build | one version of the game, written `16.19.8207193` |
| install | the game's files on a disk; its build is the **target build** of a run |
| WAD | one of the game's archives, such as `Lux.wad.client` |
| chunk | one file in a WAD, found by the hash of its lowercased path |
| bin | a property file: a list of bins it links and a list of objects |
| object | one entry of a bin: an entry hash, a class, and properties |
| entry name | the name an object's entry hash is the hash of |
| link | a bin names another bin to load with it; an object names another object by its entry hash |
| character | a directory under `data/characters/`: a champion or a sub-character |
| champion | a character the game keeps in a WAD of its own name |
| sub-character | a character in a champion's WAD: a pet, a clone, a trap |
| group | a champion and its sub-characters, which load together |
| skin | one look of a character, at a slot |
| slot | a skin's number N: `skin<N>.bin`, `Skins/Skin<N>` |
| base | the skin at slot 0, which every player has |
| premium skin | a skin at any other slot, which only its owners see |
| chroma | a recolor of a parent skin, at a slot of its own |
| skin entry | the object that defines a skin, `Characters/<C>/Skins/Skin<N>` |
| resolver | the object that maps effect keys to effects for a skin |
| graph | a skin's animation graph: the clips it plays and when |
| clip | one animation file, `.anm`; or one sound in a bank, where the bank is named |
| rig | a skin's mesh and skeleton, and the clips that move them |
| root bin | a champion's own bin, `data/characters/<c>/<c>.bin`: its spells and gameplay data |
| bank | a Wwise sound bank; a **media bank** holds clips, an **events bank** holds what plays them |
| bank unit | an entry of a skin's audio block: a name, bank paths, an events list, a voice flag |
| tag | a name in a skin's audio block from which spell sound hooks are composed |
| rework | a build at which a champion's model was replaced for every skin |
| history | the census history: one commit per build, with what every file and object was |
| index | the SQLite file built from the history, which a run asks |
| position | a build's number in the history, in the order builds arrived |
| head | the build the history took in last; the newest build is the one of the highest version |
| life | the first and the last position at which the game held a key |
| span | a life with no gap: the game held the key at every build of it |
| placement | a file a WAD held at a path, over one span |
| entry text | one bin object as ritobin text in the history, found by its blob |
| key | what the index finds a file, an object or a clip by: the head of a SHA-256 of its bytes |
| norm key | an object's key with its entry name left out: the same object under another name |
| schema | the type the target build reads for each field of each class |
| names | the tables that spell a hash: Mimir's, text tables, and what a run learns |

## A mod

| term | means |
|---|---|
| mod | what an author made and published |
| archive | the file a mod comes in: a `.fantome`, a zip of WADs and metadata |
| mod id | the first twelve hex digits of the archive's SHA-256 |
| unit | one `skin<S>.bin` a mod ships, with what the client loads for it |
| S, the source slot | the slot a unit's bin sits in |
| O, the origin | the game skin a unit's content derives from: the base, a premium skin, custom, unknown |
| T, the target | the slot a unit's output goes on |
| load | the bins the client loads for a unit: its skin bin and the mod's bins it links |
| copy | a unit that is another unit's work again at another slot |
| cluster | units that are copies of each other |
| representative | the one unit of a cluster taken as the author's |
| spread | a cluster that covers a skin other than its representative's own |
| paste | a premium skin's content saved at slot 0 |
| binless mod | a mod with no skin bin: only files at the game's paths |
| own | made by the mod's author: bytes the game never shipped |
| the game's | bytes the game ships or shipped: at this path, at another path, at an older build |
| override | a file of a mod at a path the game ships, taken as replacing the game's file |
| stale copy | a file of a mod at a path the game ships, holding the bytes the game shipped there at an earlier build; no override |
| borrowed | the game's file of another skin or champion, used by a mod on purpose |
| former path | a path the game shipped at earlier builds and does not ship |
| unused | a file of a mod that nothing loads |
| dating | the oldest build a mod can have been made at |
| restoration | a mod that brings back a champion's model from before its rework |
| family | archives that share most of their own files' bytes: versions, ports and fixes of one mod |

## A run

| term | means |
|---|---|
| run | reforge taking one archive through its stages |
| evidence | what a run established of a mod that its package does not show again: its dating, its restorations |
| port | a run that puts a mod over the game's own skin at T; also what it writes |
| repair | a run that gives the mod as it is, at its own slots and paths, fixed for the target build |
| stage | one step of a run, in order: read, load, fix, facts, decide, plan, build, check, write |
| fixup | a change that brings old or tool-damaged data to what the target build reads |
| rule | a fixup that is data: remove a field, rename fields, move fields into a struct |
| change | the record of what a fixup did at one place: the object, the field path, before and after |
| fact | something true of a mod, whatever is chosen for it |
| policy | what chooses T from the facts: a target policy and an animation-reuse level |
| candidate | a slot a unit is judged against |
| fit | a unit's mesh, submeshes, skeleton and clips work on a slot |
| decision | the T chosen for a unit, with its reason and its candidates |
| reach | what a unit's skin entry names, and what those name, within its load |
| carried | an object or a file the port ships as the mod's own |
| namespace | `<name>.<author>`, under which a port's files and objects are named |
| retarget | to rename a unit's skin entry, resolver and graph from S to T |
| package | what a run writes: layers of chunks and patches, hash tables, the project's config |
| layer | a part of a package a player turns on or off: the base layer, `voice`, `hud` |
| patch | a PTCH: records that replace fields of a game bin, applied when the mod is installed |
| check | a test of a built package: patches applied as the overlay applies them, types, a round trip |
| diagnostic | what a run found about a mod or about its inputs, with a code and a severity |
| fault | a failure of the tool or the machine; it ends the run and is no diagnostic |
| report | everything a run found and chose, serialised |
| dump | a stage's result as text |

## The corpus

| term | means |
|---|---|
| datasets | the collections of archives the corpus is drawn from, each named by its SHA-256 |
| corpus | the mods of the datasets in scope: champion skin mods, less duplicates |
| edge case | one rule with an outcome of its own: a record's case, a diagnostic code, a variant of a closed set |
| reduced corpus | the fewest mods that together show every edge case the corpus shows |
| baseline | the reports of one run of reforge over the corpus, which later runs are compared with |
| forge | ltk-forge, the retired program reforge rewrote |
| oracle | ltk-forge in the comparisons of the rewrite, which ended with its retirement |
