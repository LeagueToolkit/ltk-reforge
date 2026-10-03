# MOD-ARTIFACT-0015: Bins as text or JSON at a bin's path

- **Source:** not established
- **Affects:** a file at a path ending `.bin`, or at a path the game holds or held a file at
- **Related:** #MOD-ARTIFACT-0016, #GAME-CHANGE-0012

## What is found

A file where the client reads a binary bin, whose bytes are a bin written as text: ritobin's
text form, which begins `#PROP_text`, or ritobin's JSON form. The path ends `.bin`, or is one
the game holds or held a file at.

Every such file of the baseline run is at the path of a bin the game shares between skins:
219 at a path directly under `data/` and 2 at a `_multi_` path (#GAME-CHANGE-0012).

It is told from an author's source by its path. A bin as text under another name, such as a
`.py` file beside the binary bin, is a source and takes no bin's place (#MOD-ARTIFACT-0016).

## Why it is there

Not established. Which tool leaves the text form under the bin's name, and why at shared bins
alone, is not known.

## What reads it, and how

A reader of binary bins rejects the file: it has no bin's magic (`bin.unreadable`).

In most of these mods the file is at a path a loaded bin links, so the client is asked for it.
ltk-forge takes it that the client reads no text there and that what the file holds never
showed. No check in game is recorded.

## The repair

None. A binary bin made of the text would load. Whether the author ever saw what the text
holds is not established, so the text stays as it is.

- The file is reported (`inventory.bin-text`), and a loaded one is reported as a bin that does
  not parse (`bin.unreadable`).
- A text file at a shared bin's former path is not carried to the bin's path of the target
  build (#GAME-CHANGE-0012). A bin's path takes a bin only.

## Evidence

- The baseline run (2026-10-03): 64 of the 5,137 mods, 221 files. 148 files in 44 mods are JSON
  and 73 files in 20 mods are text.
- In 56 of the 64 mods a file the client is asked for as a bin does not parse as one: 190
  files.
- 1,914 mods of that run hold a bin as text or JSON under any name.
- ltk-forge's run of 2026-10-01 carried such files from the shared bins' former paths to their
  paths of the target build in four mods, and each failed to load there.
