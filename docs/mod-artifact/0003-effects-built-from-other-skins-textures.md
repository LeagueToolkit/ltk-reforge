# MOD-ARTIFACT-0003: Effects built from other skins' textures

- **Source:** authors
- **Affects:** textures and meshes under `particles` folders, the emitters that name them
- **Related:** #MOD-ARTIFACT-0002

## What is found

A mod's effects name textures of another skin of its champion, or of another champion: by the
game's path, or as a copy the mod ships at its own path or at the other skin's.

It is told from a paste (#MOD-ARTIFACT-0002) by what is borrowed. A paste brings a skin's entry,
rig and clips. This brings effects files only.

## Why it is there

Authors build effects from what the game has: a gradient, a noise texture, a text texture of
another skin. Riot's own skins reuse other champions' files the same way.

## What reads it, and how

The client draws the effect with the texture named, whoever's it is, when the file is there: a
file of another champion is there only when a WAD mounted with this champion holds it, or the
mod ships it.

## The repair

None. It is the author's work as meant.

## Evidence

- One mod ships 14 effects files of its champion's skin 23 and 62 from other champions' skin 8
  folders. A rule that counted them called it a paste of skin 8. It is custom work.
- Seen across ltk-forge's corpus on 2026-09-29, when its textures were first scanned: a mod of
  one champion shipping another's gradient, another shipping a third champion's text textures.
