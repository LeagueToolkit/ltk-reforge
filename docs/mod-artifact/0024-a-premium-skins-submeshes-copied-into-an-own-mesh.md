# MOD-ARTIFACT-0024: A premium skin's submeshes copied into an own mesh

- **Source:** authors
- **Affects:** skinned meshes (`.skn`), `skinMeshProperties.initialSubmeshToHide`,
  `SubmeshVisibilityEventData.mShowSubmeshList`
- **Related:** #MOD-ARTIFACT-0002, #ADR-0013

## What is found

A mesh of the mod's own bytes in which a whole submesh is a premium skin's: its vertices lie
where that skin's mesh has vertices, and not where the base's mesh has them.

It is recognised by position. Vertex positions are rounded to 1/16 unit, as the census history
rounds them. A submesh counts as copied from skin N when 90% or more of its distinct positions
are positions of skin N's mesh and are not positions of the base's mesh. A submesh the author
modelled shares few positions with any game mesh. A submesh kept from the base shares them with
the base.

Such a submesh is one of three kinds, by the skin entry and the graph the unit loads:

- visible: `initialSubmeshToHide` does not name it;
- hidden and shown: `initialSubmeshToHide` names it, and a `SubmeshVisibilityEventData` of the
  graph shows it (a recall prop, a weapon swap);
- hidden and never shown.

## Why it is there

An author builds a model from parts: their own body with a premium skin's weapon, wings or
recall prop. Exporting the premium skin's mesh and editing it leaves that skin's submeshes in
the result, some of them hidden and forgotten.

## What reads it, and how

The client draws the submesh like any other. On a skin other than N the submesh is the premium
skin's geometry on a skin whose owner did not buy it, and the animation events that show a
hidden prop are skin N's graph events.

## The repair

A mod for skin N, where the submesh is visible or shown by an event. Where it is hidden and
never shown, the mesh with the submesh removed fits any slot.

## Evidence

- The threshold of 90% and the rounding are ltk-forge's, set over its first corpus (1,029 mods,
  2026-09-29).
- Over the datasets (5,137 mods, 2026-10-04): 4,227 submeshes of 475 mods are copied from a
  premium skin by this measure.
