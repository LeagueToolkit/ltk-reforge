# MOD-ARTIFACT-0018: textureMult wrapped twice

- **Source:** a tool that converts an emitter to the layout of 13.1; which is not established
- **Affects:** `VfxEmitterDefinitionData.textureMult`,
  `VfxTextureMultDefinitionData.textureMult`
- **Related:** #GAME-BEHAVIOR-0002, #GAME-CHANGE-0005, #MOD-ARTIFACT-0017

## What is found

An emitter's `textureMult` struct whose own `textureMult` is another struct of the same class,
where the build reads a path string:

```
textureMult: pointer = VfxTextureMultDefinitionData {
    textureMult: pointer = VfxTextureMultDefinitionData {
        textureMult: string = "..."
        texDivMult: vec2 = { ... }
    }
}
```

The path and the placing and scrolling fields are in the inner struct, in the outer one, or
split between the two.

It is told from deliberate work by the type: `VfxTextureMultDefinitionData.textureMult` is a
string at every build that has the class, and no game emitter holds a struct there.

## Why it is there

13.1 turned the emitter's `textureMult` from a path string into a struct whose own
`textureMult` is the path (#GAME-CHANGE-0005). The field keeps its name at both levels.

The shape is that of a tool that wrapped every `textureMult` string it found in the struct, the
struct's own path included: a conversion applied to an emitter that already had the new layout.
That is inferred from the shape. Which tool did it is not established.

## What reads it, and how

The client reads the outer struct. It reads a string in that struct's `textureMult`, finds a
struct, and drops it (#GAME-BEHAVIOR-0002). The path and every field of the inner struct are
lost with it, and the second texture is not drawn.

The author's own build dropped it the same way, where the mod was made at 13.1 or later.

## The repair

One struct: the emitter's `textureMult` as a `VfxTextureMultDefinitionData` whose `textureMult`
is the path string, holding the fields of both levels.

## Evidence

- The baseline run (2026-10-03), in the objects ltk-forge's port repaired: 6 of the 5,137 mods
  hold it, in 1,088 emitters. In 2 mods dated at builds that read a string there, 459 more such
  structs are counted as never read by the mod's own build.
- The meta database (generation 2026-09-28, read 2026-10-04): from 13.15 to 16.19
  `VfxEmitterDefinitionData.textureMult` is a pointer to `VfxTextureMultDefinitionData`, and
  `VfxTextureMultDefinitionData.textureMult` is a string.
- Counted in the census history's entry texts by the field's hash and the line's depth
  (2026-10-04), at 13.1.4875491, 13.15.5229820, 14.1.5516065 and 16.19.8207193: every
  `textureMult` that is a pointer is at an emitter's depth, and every one at the depth of the
  struct's fields is a string. At 16.19.8207193 it is 420,962 pointers and 403,320 strings.
- That the author's build dropped the inner struct is inferred from #GAME-BEHAVIOR-0002. No
  check in game is recorded.
