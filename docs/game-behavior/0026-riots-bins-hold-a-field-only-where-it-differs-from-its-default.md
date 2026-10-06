# GAME-BEHAVIOR-0026: Riot's bins hold a field only where it differs from its default

- **Status:** Inferred
- **Builds:** 8.20 to 16.19 in the census history; 16.17 in LTK Manager's counts
- **Affects:** every bin class
- **Related:** #GAME-BEHAVIOR-0021, #GAME-CHANGE-0017, #MOD-ARTIFACT-0020

## Behaviour

A bin of the game holds a property only where its value differs from the default the schema
gives the field. A field a game object does not hold has its default there.

A pointer to a struct of no fields is held where the default is a null pointer: it differs from
the default, and its struct holds the defaults of its own fields.

The rule is of the tool that writes Riot's bins. A mod's bins hold what its author or its tool
wrote, defaults included.

Whether the client's own default is always the schema's is open. LTK Manager's reading of the
client has its registrar declare `UseLingerRotation` on, where the schema gives `false`, and the
game's bins hold the field only as `true`.

## In a mod

A field no game object holds is not shown by that to be a field no build declared: every game
object may hold its default. The meta schema tells the two apart from 13.15. Before 13.15 the
census alone does not.

A copy of a game object that holds a field at its default differs from the game's object there,
and draws the same.

## Evidence

- Counted in the census history's object texts by the field's hash (2026-10-07), at 11 builds
  from 8.20.2483196 to 16.19.8207193. Each flag is held only at the value that is not its
  default (`rito-meta`, generation 2026-10-05). At 16.19.8207193:

  | field | default | held as | lines |
  |---|---|---|---:|
  | `VfxEmitterDefinitionData.isLocalOrientation` | `true` | `false` | 400,233 |
  | `VfxEmitterDefinitionData.IsEmitterSpace` | `false` | `true` | 5,426 |
  | `VfxEmitterDefinitionData.meshRenderFlags` | 1 | 0 | 195,860 |
  | `VfxTextureMultDefinitionData.uvScrollAlphaMult` | `true` | `false` | 6,549 |
  | `VfxTextureMultDefinitionData.isRandomStartFrameMult` | `false` | `true` | 8 |
  | `VfxLingerDefinitionData.UseLingerRotation` | `false` | `true` | 1,483 |

  The same holds at each of the other 10 builds where the field is held. The class of a line
  was not read for this count; the hashes are the fields' own.
- `0x6aec9e7a` of #GAME-CHANGE-0025, for which the schema gives no default, is held only as
  `true`: 4,951 lines at 16.19.8207193.
- Pointers to a struct of no fields, the same count at 16.17.8104348: 28 `flexBirthTranslation`
  and 167 `flexScaleEmitOffset`, whose default is a null pointer.
- LTK Manager's VFX renderer plan (read 2026-10-07), over the 16.17 dump of `data/characters`,
  15,574 emitters (its section 2.10): `pass`, `miscRenderFlags`, `alphaRef`, `isUniformScale`
  and `particleLinger` are off their default in every emitter that holds them. Over the 174
  champion WADs: no `birthDrag` at its default (section 2.30), `depthBiasFactors` held only
  where it is not zero (section 2.37), and `isLocalSpace`, default `true`, held only as `false`
  on 1,956 orbital and 6,427 acceleration fields (section 2.36).
- The plan's tier T8 states the registrar's default of `UseLingerRotation` and the schema's.
- #GAME-CHANGE-0017: at 13.7.5031019, none of the 2,312 primitives that hold an old align flag
  holds `false`.
- What would confirm it field by field: every field of the install's bins counted against the
  schema's default.
