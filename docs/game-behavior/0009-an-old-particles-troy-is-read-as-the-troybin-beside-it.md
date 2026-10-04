# GAME-BEHAVIOR-0009: An old particle's .troy is read as the .troybin beside it

- **Status:** Inferred
- **Builds:** 10.9 to 16.19
- **Affects:** a path string ending `.troy`
- **Related:** #GAME-BEHAVIOR-0007

## Behaviour

Where an object names a particle file ending `.troy` and no file is at that path, the client
reads the file at the same stem ending `.troybin`.

## In a mod

- A reference to a `.troy` path with no file there is no loose end when the game holds the
  `.troybin`.

## Evidence

- Every skin entry of one champion in the game's own bins names
  `DATA/Particles/Malphite_Glacial_Idle.troy`, a path the game never shipped. The `.troybin` at
  that stem has shipped since 10.9, and the champion's skins play.
- Inferred from the game's own data and not checked in game. What would confirm it: the effect
  seen in game with the `.troybin` replaced by a mod.
