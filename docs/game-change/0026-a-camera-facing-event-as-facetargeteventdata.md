# GAME-CHANGE-0026: A camera-facing event as FaceTargetEventData

- **Build:** 13.8.5036621
- **Affects:** the events of a clip, `BlendableClipData.mEventDataMap`, of the class
  `FaceCameraEventData` (`0x76b94c49`)
- **Related:** #GAME-BEHAVIOR-0028

## Before

Up to 13.7 an event that turns the unit during a clip is a `FaceCameraEventData`, held in the
clip's `mEventDataMap` under the event's name. It holds `YRotationDegrees: f32`, and in a few
graphs `FaceTarget: u8`.

## After

From 13.8 the same event is a `FaceTargetEventData` (`0xd7a6b107`), a `BaseEventData` that
declares `BlendInTime`, `BlendOutTime`, `FaceTarget: u8` (default `0`) and
`YRotationDegrees`. No build holds a `FaceCameraEventData`.

## Riot's conversion

The class hash alone. Each event keeps its name, its place in the map and every value it held.
Riot set no `FaceTarget` on an event that had none: the default is the camera's.

## In a mod

A mod made before 13.8 holds the old class. The client from 13.8 does not declare it, and a
graph that holds such an event crashes the client at load (#GAME-BEHAVIOR-0028).

## Evidence

- The census history's entry texts at 13.7.5031019 and 13.8.5036621 (read 2026-10-09). At
  13.7.5031019, 725 files hold 1,268 structs of `0x76b94c49` and none of `0xd7a6b107`. At
  13.8.5036621, none of `0x76b94c49`, and 736 files hold 1,286 of `0xd7a6b107`.
- The same structs paired across the two builds, by file and by order in the file: each of the
  1,268 is the struct at 13.8.5036621 with its class hash changed, and nothing else.
- At 13.7.5031019, 2 of the 1,268 hold `FaceTarget`, both `1`, both in Milio's graph.
- Yasuo's graph at 13.7.5031019 and 13.8.5036621: the event `FaceCamera` of the clip
  `Dance_IN_RAW`, `YRotationDegrees: 135`, under each class.
- The meta database (generation 2026-10-05, read 2026-10-09) declares no `FaceCameraEventData`
  from 13.15 on, and declares `FaceTargetEventData` with the four fields above.
- The old class's name is the hash tables' for `0x76b94c49`.
