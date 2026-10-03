# MOD-ARTIFACT-0005: Banks added with events of their own

- **Source:** authors
- **Affects:** `.bnk`, `.wpk`, the skin entry's `skinAudioProperties.bankUnits`, literal event
  names in animation clips and emitters
- **Related:** none

## What is found

A media bank at a path no bank unit of the game loads (one mod of a champion ships
`<other name>_sfx_audio.bnk`), listed in a bank unit the mod added, beside an events bank of the
mod's own. No event of the game's events banks plays its clips.

The events have names the author made (`Play_sfx_<name>_recall`), which the mod's animations and
effects name literally. No such event shares an id with an event of the game's banks.

## Why it is there

The author added sounds the champion never had: for a model of their own, a recall, an emote.
The game's events name the game's clips, so new sounds need new events.

## What reads it, and how

The client loads the unit's banks and plays an event when an animation clip or an emitter names
it. An events bank of an older version than the client's does not load, and its sounds are
silent.

## The repair

The mod's own clips in a media bank, with the events of the mod's events bank that reach them.

## Evidence

- ltk-forge's seventh datasets run (2026-10-01): 153 media files of this kind holding 1,371
  clips, 135 of them beside an events bank of the version the client loads.
- Checked in game on 2026-10-01: one sample's added sounds play from a port that carries them
  so.
