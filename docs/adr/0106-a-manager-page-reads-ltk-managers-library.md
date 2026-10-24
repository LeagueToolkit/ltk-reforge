# ADR-0106: A Manager page reads LTK Manager's library for the batch

- **Status:** Accepted (2026-10-24)
- **Related:** #ADR-0046, #ADR-0091, #ADR-0105, #MOD-ARTIFACT-0040;
  [docs/design/ROADMAP-UX.md](../design/ROADMAP-UX.md), "The Manager page"

## Context

A person who has used LTK Manager for a while holds a library of plain Fantome archives, the
ones without league-mod's layer directories and game data (#MOD-ARTIFACT-0040). Porting them
one by one means finding each archive by hand. The Batch page runs a list of archives through
write into one folder (#ADR-0091), so what it lacks is the list.

The manager keeps its library as files in its data folder: `library.json`, one document with
every installed mod's id, slug, format and storage, and under `mods/` each mod's folder, with
its config and thumbnail, and beside it the archive the mod arrived as when the mod is stored
as one. A mod the person unpacked into a project has no archive. An older layout kept the
archives under `archives/` by id. Beside the index, `wad-reports.json` holds what the manager
derived of each mod from the game WADs it touches: its champions, its maps and its tags,
`champion-skin` among them. reforge ports champion mods alone.

The options were to read those files, to ask the manager through an interface it does not
have, or to leave the person to find the archives.

## Decision

A page of its own, third in the sidebar, lists the manager's library at the window's whole
width and hands the archives ticked to the Batch page (the user's call, 2026-10-24: a page
with room to pick, not a dialog on the Batch page). The app reads `library.json` and each
mod's config from the manager's data folder, the folder Tauri names for the manager's
identifier beside the app's own, and lists every mod: its name, its format, its champions as
the manager's report names them, whether it has an archive, and whether the archive is a plain
Fantome or carries the extensions. The plain Fantomes of champions with an archive are ticked
first; a filter narrows the list for picking a few. The archives ticked join the batch's list
as picked files do, and the batch runs as the Batch page runs one.

The read is read only. Nothing under the manager's folder is written, renamed or locked, so
the manager may run at the same time. A mod's name is the manager's text and reaches the page
through `clean` (#ADR-0046). An archive is told extended by its entries alone: a `WAD_<layer>/`
or `META/game_data/` entry. A hash table under `META/hashes/` does not make it so, since the
manager writes one into every archive it imports (#MOD-ARTIFACT-0040).

## Consequences

- A library of plain Fantomes is ported in one batch into the output folder.
- A mod stored as a project is listed and cannot be added: the app reads archives, and
  packing the manager's project is the manager's to do.
- A `library.json` of a shape the app does not know lists what it can read and nothing more;
  a missing folder says the manager is not installed.
- The list is as the files stand when the dialog opens. A mod installed while it is open is
  not shown until it is opened again.
