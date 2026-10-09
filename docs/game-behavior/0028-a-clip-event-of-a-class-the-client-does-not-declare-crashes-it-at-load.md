# GAME-BEHAVIOR-0028: A clip event of a class the client does not declare crashes it at load

- **Status:** Confirmed in game (2026-10-09)
- **Builds:** 16.19
- **Affects:** the events of a clip, `BlendableClipData.mEventDataMap`
- **Related:** #GAME-BEHAVIOR-0002, #GAME-CHANGE-0026

## Behaviour

An animation graph with an event of a class the client does not declare crashes the client
while it loads the champion, before the loading screen closes. The event is not skipped as a
field of another type is (#GAME-BEHAVIOR-0002).

The game log says nothing of it. It ends at the marker of loading step 52, which mounts the
champions' archives, with no error line and no line of the crash handler.

Not established: whether a struct of an undeclared class elsewhere in a bin crashes the client
too.

## In a mod

A graph made before 13.8 with a `FaceCameraEventData` event crashes the game from 13.8 on
(#GAME-CHANGE-0026). The rest of the mod, the mesh, the skeleton and the effects, loads.

## Evidence

- Seen in game (2026-10-09, the 16.19.8207193 copy, a replay of Yasuo at skin 0). The port of a
  Yasuo mod whose graph holds one `FaceCameraEventData` event, and three cuts of it, each with
  the port's patch of the skin bin unchanged:
  - the port as it was made: the game crashed;
  - its bin of effects emptied: the game crashed;
  - its graph swapped for the game's graph of the slot: the game loaded;
  - its graph with that one event's class changed to `FaceTargetEventData`, its values kept:
    the game loaded.
- Each crash's log ends at `SEJ-9F31B5D0`, 1.4 s in, after the load's earlier markers.
  LTK Manager's table of log codes names `SEJ-9F31B5D0` loading step 52, mounting the
  champions' archives. The log of a load that finishes goes on to `SEJ-4A7C2E89`, step 53.
- The mod's own skeleton and mesh under the game's graph loaded in game the same day, so the
  graph and not the rig is what crashed.
