# ADR-0105: A package is also written as a layered Fantome archive

- **Status:** Accepted (2026-10-24)
- **Related:** #MOD-ARTIFACT-0040, #ADR-0026, #ADR-0041, #ADR-0051, #ADR-0102,
  [docs/design/ARCHITECTURE.md](../design/ARCHITECTURE.md), "Package"

## Context

A package is written as a mod project directory and, when asked, packed as a `.modpkg`. Most
mods reach a person as a `.fantome`, and LTK Manager reads a Fantome archive with league-mod's
layer directories and game data as a whole project (#MOD-ARTIFACT-0040): its layers, its PTCH
and its program all arrive. league-mod's packer writes that archive from the same project
directory the project writer makes, so a third writer costs the packing call and a file name.

What the layout cannot do: a manager that predates the extensions loads the base layer alone.
A port is a PTCH on the game's bin (#ADR-0026), which such a manager copies and never applies,
so the port loads without its objects and nothing says so. A `.modpkg` is refused there
outright. A repair writes whole bins at the mod's own paths in the base layer, and installs in
either.

The thumbnail is re-encoded as PNG by the packer, the project's `README.md` becomes
`META/README.md`, and the two hash tables go under `META/hashes/`.

## Decision

A package is also written as `<out>/<project>_<version>.fantome`, with the report beside it
as `<project>_<version>.reforge-report.json`, when the caller asks for it: the project staged,
packed by league-mod's Fantome packer and removed, as the `.modpkg` is. The command line asks
with `--fantome`, beside `--modpkg`. The app writes one form, chosen under Settings
(`output.format`), and the `.fantome` is the default (the user's call, 2026-10-24).

The archive is a target for LTK Manager. It is not offered as a Fantome that works in every
manager: the writer's documentation, the command's help and the setting's description say that
a port's patch applies in LTK Manager and league-mod alone.

## Consequences

- A person who keeps a `.fantome` library gets a port they can drop in it.
- The league-mod crates come from crates.io, `ltk_mod_project` with its `fantome` feature, and
  `ltk_fantome` is a dependency for reading an archive back.
- A port written as a `.fantome` and installed in cslol-manager loads the base layer's files
  and not the patched bin. Which managers a person uses is theirs to know: the setting says
  so, and a person on another manager chooses the `.modpkg`.
- The app opens a `.modpkg` as a run by its embedded report; a `.fantome` it wrote is not
  opened so yet.
- Two writes of one package give the same bytes, which rests on `ltk_fantome` 0.15.5's
  deterministic build; a test holds it.
