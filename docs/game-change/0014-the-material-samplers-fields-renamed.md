# GAME-CHANGE-0014: The material sampler's fields renamed

- **Build:** 15.3.6535410 for the texture's path, 15.4.6568035 for the slot's name;
  `samplerName` holds another value from 15.21.7194082 to 16.12.7884269
- **Affects:** `StaticMaterialShaderSamplerDef`, the samplers of a `StaticMaterialDef`
- **Related:** #GAME-BEHAVIOR-0002, #GAME-CHANGE-0007, #GAME-CHANGE-0013

## Before

Up to 15.2 a sampler of a material holds two strings:

- `samplerName`, the name of the shader's slot: `"Diffuse_Texture"`.
- `TextureName`, the path of the texture bound to the slot.

## After

From 15.4 a sampler holds:

- `TextureName`, the name of the shader's slot.
- `texturePath`, the path of the texture. It is a string up to 16.16 and a `file` from 16.17
  (#GAME-CHANGE-0013).

`TextureName` keeps its name and changes its meaning: a path up to 15.2, a slot's name from
15.4.

At the builds of 15.3 a sampler holds `samplerName`, the slot's name, and `texturePath`, the
path. It holds no `TextureName`.

From 15.21.7194082 to 16.12.7884269 some samplers hold `samplerName` again, beside
`TextureName` and `texturePath`. Its value there is the name of a sampler state, `"Wrap_No_Mip"`
or `"Clamp_No_Mip"`, not a slot's name. No sampler holds it from 16.13.7888140.

| build | samplers | `samplerName` | `TextureName` | `texturePath` |
|---|---:|---:|---:|---:|
| 13.15.5229820 | 39,194 | 39,194 | 39,078 | 0 |
| 15.2.6525532 | 62,334 | 62,334 | 62,227 | 0 |
| 15.3.6535410 | 62,272 | 62,272 | 0 | 62,165 |
| 15.3.6564086 | 62,272 | 62,272 | 0 | 62,165 |
| 15.4.6568035 | 62,781 | 0 | 62,781 | 62,674 |
| 15.6.6649480 | 64,923 | 0 | 64,923 | 64,816 |
| 16.12.7884269 | 98,159 | 793 | 98,159 | 97,956 |
| 16.13.7888140 | 99,216 | 0 | 99,216 | 99,013 |
| 16.16.8049184 | 112,408 | 0 | 112,408 | 112,191 |
| 16.17.8104348 | 111,573 | 0 | 111,573 | 111,356, each a `file` |

## Riot's conversion

| up to 15.2 | at 15.3 | from 15.4 | holds |
|---|---|---|---|
| `samplerName` | `samplerName` | `TextureName` | the slot's name |
| `TextureName` | `texturePath` | `texturePath` | the texture's path |

- **15.3.6535410.** The path moved from `TextureName` to `texturePath`. A sampler with no path
  got no `texturePath`. No other field of a sampler changed its name.
- **15.4.6568035.** The slot's name moved from `samplerName` to `TextureName`, its value
  unchanged.
- **16.13.7888140.** Each of the 793 samplers that held a sampler state's name in `samplerName`
  lost it. Nothing took its place and no other field of those samplers changed.

## In a mod

- **A mod made before 15.3** holds the slot's name in `samplerName` and the path in
  `TextureName`. A build from 16.13 does not declare `samplerName` and drops it
  (#GAME-BEHAVIOR-0002). It reads `TextureName` as the slot's name, so the path is taken for a
  slot the shader does not have, and the sampler binds no texture.
- **A mod made at 15.3** holds the slot's name in `samplerName` and the path in `texturePath`.
  A build from 16.13 drops `samplerName`, and the sampler names no slot.
- **A mod made from 15.21 to 16.12** can hold a sampler state's name in `samplerName` beside a
  slot's name in `TextureName`. A build from 16.13 drops it.

The two meanings of `samplerName` are told apart by `TextureName`. Where a sampler holds
`samplerName` and its `TextureName` is a path or absent, `samplerName` is the slot's name. Where
its `TextureName` is a slot's name, `samplerName` is a sampler state's name.

That the client binds no texture for a sampler in an old layout is inferred from
#GAME-BEHAVIOR-0002 and from what each field holds. No check in game is recorded.

## Evidence

- Counted in the census history's entry texts by the class's and the fields' hashes
  (2026-10-04): the table above.
- The same samplers paired across 15.2.6525532 and 15.3.6535410: 62,261 samplers are in both.
  `samplerName` is equal in each. Of the 62,154 that hold a path, the new `texturePath` equals
  the old `TextureName` in 57,558, and with the extension set aside in 62,125: that build moved
  the textures of the champions Aatrox to Dr. Mundo from `.dds` to `.tex` (#GAME-CHANGE-0007).
  The other 29 name another file. The other fields of the sampler are equal in 62,199.
- Paired across 15.3.6564086 and 15.4.6568035: 62,272 samplers are in both, and in each the new
  `TextureName` equals the old `samplerName`.
- Paired across 16.12.7884269 and 16.13.7888140: the 793 samplers that hold `samplerName` are
  in both builds, and each is equal at 16.13.7888140 but for that field. 632 hold
  `"Wrap_No_Mip"` and 161 `"Clamp_No_Mip"`.
- By the field's hash: no sampler holds `samplerName` at the builds sampled from 15.4.6568035
  to 15.20.7190545. 8 entries hold it at 15.21.7194082.
- The meta database (generation 2026-09-28, read 2026-10-04), by build:
  `StaticMaterialShaderSamplerDef.samplerName` is a string from 13.15.5229820 to 15.3.6564086
  and from 15.6.6649480 to 16.12.7884269, and is not declared between or after.
  `TextureName` is a string up to 15.2.6521536 and from 15.4.6575386, and is not declared at the
  builds of 15.3. `texturePath` is a string from 15.3.6540407, the first build of 15.3 the
  database holds, to 16.16.8049184, and a `file` from 16.17.8104348.
- The class declared `samplerName` from 15.6.6649480, and the first game sampler to hold it
  after that is at 15.21.7194082.
- The baseline run (2026-10-03), in the objects ltk-forge's port repaired: 290 of the 5,137
  mods hold samplers with `samplerName`, 7,546 samplers. The count does not tell the two
  meanings apart.
