# MOD-ARTIFACT-0017: Emitter fields nested in textureMult

- **Source:** a tool that converts an emitter to the layout of 13.1; which is not established
- **Affects:** `VfxEmitterDefinitionData.textureMult`, `VfxTextureMultDefinitionData`
- **Related:** #GAME-BEHAVIOR-0002, #GAME-CHANGE-0004, #GAME-CHANGE-0005, #MOD-ARTIFACT-0018

## What is found

An emitter's `textureMult` struct, a `VfxTextureMultDefinitionData`, that holds fields the
emitter itself held in the layout up to 12.18: `birthScale1`, `particleBind`, `scale1`,
`orientation`, `birthRotation1`, `birthRotationalVelocity1`.

```
textureMult: pointer = VfxTextureMultDefinitionData {
    textureMult: string = "..."
    birthScale1: embed = ValueFloat { ... }
    particleBind: vec2 = { ... }
}
```

In most of the mods the struct is also wrapped twice (#MOD-ARTIFACT-0018), and the fields sit in
the outer struct or the inner one.

It is told from deliberate work by the class. `VfxTextureMultDefinitionData` declares eighteen
fields (#GAME-CHANGE-0005) and none of these, and no texture mult of the game holds one.

## Why it is there

Up to 12.18 an emitter held these fields beside its texture-mult fields. 12.19 moved them into
`LegacySimple` (#GAME-CHANGE-0004), and 13.1 moved the texture-mult fields into `textureMult`
(#GAME-CHANGE-0005).

The shape is that of a tool that built the 13.1 struct from an emitter older than 12.19 and
moved the simple-particle fields into it with the texture-mult fields. That is inferred from
the shape. Which tool did it is not established.

## What reads it, and how

The client drops a field its class does not declare (#GAME-BEHAVIOR-0002). The struct's own
fields are read, and the particle is drawn with the default scale, rotation and binding.

The author's own build dropped them too: the class has declared none of them at any build.

## The repair

The fields in the emitter's `LegacySimple`, under the names 12.19 gave them
(#GAME-CHANGE-0004): `birthScale1` as `birthScale`, `particleBind` as `particleBind`.

## Evidence

- The baseline run (2026-10-03), in the objects ltk-forge's port repaired: 9 of the 5,137 mods
  hold such fields, in 165 emitters. `birthScale1`: 164 in 9 mods. `particleBind`: 164 in 9.
  `birthRotationalVelocity1`: 48 in 3. `scale1`: 44 in 6. `orientation`: 41 in 6.
  `birthRotation1`: 23 in 5.
- The meta database (generation 2026-09-28, read 2026-10-04): from 13.15 to 16.19
  `VfxTextureMultDefinitionData` declares the eighteen names of #GAME-CHANGE-0005 and no other
  field.
- Counted in the census history's entry texts by the fields' hashes (2026-10-04), at
  13.1.4875491, 13.15.5229820, 14.1.5516065 and 16.19.8207193: no line holds `birthScale1`,
  `scale1`, `birthRotation1` or `birthRotationalVelocity1`, in any class. Every `particleBind`
  is in a `VfxEmitterLegacySimple`, and so is every `orientation` that is a number (class read
  at the first, second and fourth of those builds).
- The class does not exist before 13.1 (#GAME-CHANGE-0005), so no earlier build declared the
  fields on it.
- Whether the author meant the values is not established: the author's build did not draw
  them. No check in game is recorded.
