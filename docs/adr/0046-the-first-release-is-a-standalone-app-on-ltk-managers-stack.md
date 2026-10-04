# ADR-0046: The first release is a standalone app on LTK Manager's stack

- **Status:** Accepted (2026-10-04)
- **Related:** #ADR-0001, #ADR-0011, #ADR-0047, #ADR-0048; `docs/design/ARCHITECTURE.md`,
  "Callers"; `docs/design/ROADMAP.md`

## Context

`reforge` is a library with a command as its one caller. The fixer is built and checked in
game, and the library gives a caller what a UI needs: a run a stage at a time, every result as
data, the fixups and checks as lists with their records, a selection of fixups as JSON, and
each change a fixup made with the value before and after.

A UI could be a feature of LTK Manager or a program of its own. LTK Manager is a Tauri 2 app
with a React and TypeScript front end: Vite, Tailwind, Base UI components, TanStack router,
query, form and table, zustand, zod, paraglide for its messages, and `tauri-specta` for typed
commands between the two sides. It has a design system and guidance for both sides.

## Decision

The first release of reforge is a standalone desktop app (the user, 2026-10-04). It is built
on the framework and the libraries LTK Manager uses, and follows LTK Manager's guidance where
a choice of UI library or convention comes up.

The app is a caller of `reforge` as the command is: it drives a `Repairer` or a `Porter`, and
holds no rule of its own about a mod.

## Consequences

- The app is a crate of this workspace beside `reforge-cli`, with its front end beside it.
  `reforge` stays free of Tauri and of anything a UI needs alone.
- What the library gives as Rust types reaches the front end as generated TypeScript types, so
  a report part and its view do not drift.
- A stage runs to its end. A UI that shows progress inside a stage, or stops one, needs the
  library to report and to be interrupted, which is not built.
- The report is JSON for a program. What a person reads of a run is the app's to render.
- The inputs a run needs are the app's to find, fetch and keep: the game install, the hash
  tables (#ADR-0048), the schema database and the history index (#ADR-0047).
- A release is installed and updated as LTK Manager's is, so the same signing and updater
  questions come with it.
