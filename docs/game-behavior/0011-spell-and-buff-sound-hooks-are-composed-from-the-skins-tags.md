# GAME-BEHAVIOR-0011: Spell and buff sound hooks are composed from the skin's tags

- **Status:** Confirmed in game (2026-09-23 to 2026-09-26, and 2026-09-29)
- **Builds:** the live builds of those days; the numbers were not recorded
- **Affects:** a skin entry's `skinAudioProperties.tagEventList` and `bankUnits`; Wwise event
  ids
- **Related:** #GAME-BEHAVIOR-0012, #GAME-BEHAVIOR-0016, #ADR-0028

## Behaviour

A skin entry's `skinAudioProperties` holds `tagEventList`, a list of tags, and `bankUnits`.
A bank unit has a name, the paths of its banks, a list of event names and a voice-over flag.
Nothing in a bin reaches inside a bank.

For a spell or a buff the client composes an event's name from each tag of the list,
`Play_sfx_<tag>_<Spell>_<Hook>`, and posts it. An animation clip and an emitter name an event
literally instead.

An event's id is the 32-bit FNV-1 hash (multiply, then xor) of its lowercased name. An event is
found by its id among every bank loaded.

## In a mod

- An event renamed under a tag the entry's list holds is found by the hooks composed under
  that tag.
- A tag added after the game's adds hooks. It takes none of the game's away: the hooks under
  the game's tags are still composed (#GAME-BEHAVIOR-0016).
- A literal name in a clip or an emitter is not composed. It names one event, and plays
  nothing when no loaded bank defines it.

## Evidence

- The rule was settled in game between 2026-09-23 and 2026-09-26, before ltk-forge. ltk-forge's
  audio design states it and holds no account of the single checks.
- Checked in game on 2026-09-29, on a port of one sample to skin0: 10 own clips of a premium
  skin's bank, 9 events kept and renamed under a tag of the port's own, added after the
  game's. The port sounds as the mod does. The spell hooks composed under the later tag play
  the renamed events.
- The hash: `braum_base_sfx_audio` hashes to 3580963463, the id the game's bank of that name
  carries (#MOD-ARTIFACT-0023).
