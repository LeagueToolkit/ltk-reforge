"""The edge cases the reports of runs show (#ADR-0045), each with how many mods show it.

cases.py --out DIR [--codes FILE] [--doc FILE --dated DATE] [--jobs N] NAME=RUN [NAME=RUN ...]

A case is a key derived from a report: a diagnostic code, a value of a closed set at a place
of the report, a field that is set or a count above zero. Most keys come from the table
`RULES`: a place of the report, written as a path, and how its values make cases. A value the
program adds to a closed set shows up as a new case with no change here. A place of a report
that holds text or a flag and that neither `RULES` nor `FREE`, `OPEN` and `SKIP` name is
printed and listed in unknown.txt, so that the table is brought up to date. `RULES` also
holds, for a closed set, the values the program declares: the ones no mod shows are listed.

Inputs:

  NAME=RUN   a run directory of `run.py`, under the name of its command (`port=...`,
             `repair=...`). A mod shows a case when its report of any run does
  --codes    the table of diagnostic codes (default: docs/design/CODES.md of this checkout)
  --doc      a document whose generated tables are written again, between the marker lines
             `<!-- cases.py: cases -->`, `<!-- cases.py: unseen -->` and `<!-- cases.py: end -->`
  --dated    the date of the runs, written in the document's tables

Outputs, in DIR:

  cases.tsv    one line per case: area, case, mods, occurrences, the runs that show it, and
               up to five mods that show it: those showing the fewest cases, then by SHA-256
  unseen.tsv   the cases no mod shows: the diagnostic codes of `--codes` and the declared
               values of closed sets
  unknown.txt  the places and the values the table does not know
  mods.json    the cases each mod shows, for `reduce.py`"""
import argparse, collections, json, os, re, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import corpus  # noqa: E402

# How a place of a report makes cases:
VALUE = "value"  # each value is a case, `name=value`
FLAG = "flag"  # a case where the value is true
SET = "set"  # a case where the value is not null, and a list or a map is not empty
UNSET = "unset"  # a case where the value is null
POSITIVE = "positive"  # a case where the number is above zero
KEYS = "keys"  # each key of a map with a value that is not zero is a case, `name=key`

# The areas, in the order a run goes through them.
AREAS = ["run", "archive", "load", "fixups", "facts", "decision", "plan", "audio", "package", "checks", "records"]
# The area of a diagnostic, by the first part of its code.
CODE_AREAS = {
    "install": "run", "names": "run", "history": "run", "schema": "run",
    "archive": "archive", "inventory": "archive", "wad": "archive",
    "character": "load", "file": "load", "bin": "load", "rework": "load",
    "fixup": "fixups",
    "catalog": "facts", "copies": "facts", "origin": "facts", "rig": "facts",
    "target": "decision",
    "unit": "plan", "object": "plan", "ref": "plan", "hud": "plan",
    "audio": "audio",
    "package": "package", "write": "package",
    "check": "checks",
}  # fmt: skip

FIXUPS = (
    "known-artifacts-static-material known-artifacts-value-color known-artifacts-vfx-emitter "
    "known-artifacts-vfx-system vfx-texture-mult-strays vfx-flex-shape vfx-filtering vfx-linger "
    "vfx-legacy-simple vfx-texture-mult material-default-technique vfx-mesh-align vfx-spawn-shape "
    "vfx-spawn-shape-birth-translation material-sampler-fields vfx-emission-surface reshape-hoist "
    "reshape-wrap retype strip-undeclared"
)
REASONS = "base paste-cleaned slot fallback clips-origin skeleton-origin forced infeasible sub-follows sub-left-to-base unjudged superseded conflict pre-rework"
LEFT = "copy superseded conflict pre-rework sub-left equal-to-game no-target game-skin chroma-folded off-target"
CHECK_WHY = (
    "no-such-skin no-mesh slot-mesh other-skin-mesh own-mesh foreign-mesh copied-visible copied-shown "
    "copied-hidden no-copied-submesh no-skeleton slot-skeleton slot-joints other-skin-skeleton premium-skeleton unbound "
    "own-skeleton foreign-skeleton no-entry no-clips clips-blocked clips-fit"
)
VISIBILITY = "visible hidden-shown hidden-never"
RIG_SOURCE = "mod game missing"
CONVERSIONS = "dds-as-tex sco-as-scb as-is"
TEX_REFUSALS = "unreadable format not-one-surface too-large partial-blocks partial-mips truncated not-read-back"


def layer(name):
    """A layer's kind: a locale's voice layer is one kind, whatever the locale."""
    return "voice-locale" if name.startswith("voice-") else name


def fate(text):
    """The second of a reached object's pair: the first is the object's id."""
    return None if text.startswith("%") else text


# area, case name, how, path, the values the program declares, a function folding a value.
# In a path `[]` is an item of a list and `*` a key that is an id, a hash or a number.
RULES = [
    ("archive", "wad role", VALUE, ".wads[].role", "champion locale other"),
    ("archive", "wad layout", VALUE, ".wads[].layout", "loose packed"),
    ("archive", "wad a locale's, holding the whole mod", SET, ".wads[].packed_locale"),
    ("archive", "wad restores a model from before a rework", FLAG, ".wads[].restoration"),
    ("archive", "inventory finding", VALUE, ".inventory.items[].finding", "executable bin-text nested-archive source os-artifact unknown"),
    ("archive", "inventory item", VALUE, ".inventory.items[].kind"),
    ("archive", "inventory confidence", VALUE, ".inventory.items[].confidence", "magic heuristic name"),
    ("archive", "archive holds", KEYS, ".inventory.by_kind"),
    ("load", "file kind", VALUE, ".files[].kind", "bin ptch skn skl anm scb sco tex dds bnk wpk other"),
    ("load", "file use", VALUE, ".files[].use", "unused former-path override stale referenced loaded"),
    ("load", "file source", VALUE, ".files[].source", "archive made rewritten game"),
    ("load", "file made by", VALUE, ".files[].by", "legacy-format moved-asset bank-id"),
    ("load", "file with no path spelled", UNSET, ".files[].path"),
    ("load", "character kind", VALUE, ".characters[].kind", "champion sub"),
    ("load", "character equal to the game's", FLAG, ".characters[].equal_to_game"),
    ("load", "character with a root bin", SET, ".characters[].root_bin"),
    ("load", "unit whose bin does not read", UNSET, ".units[].bin"),
    ("load", "unit with no skin entry", UNSET, ".units[].entry"),
    ("load", "unit whose entry name spells no slot", UNSET, ".units[].entry_name_slot"),
    ("load", "dating with no bound", UNSET, ".dating.not_before"),
    ("load", "dating evidence", VALUE, ".dating.evidence.piece", "file object"),
    ("load", "provenance origin", VALUE, ".provenance[].origin", "install history own"),
    ("load", "provenance at path", VALUE, ".provenance[].at_path", "not-game-path same-bytes other-bytes"),
    ("load", "provenance still shipped", VALUE, ".provenance[].still_shipped", "true false"),
    ("load", "provenance in a skin's folder", SET, ".provenance[].skin_folder"),
    ("facts", "copy role", VALUE, ".facts.copies[].role", "representative copy"),
    ("facts", "copy kind", VALUE, ".facts.copies[].kind", "byte-identical renamed near same-rig"),
    ("facts", "copies spread over other skins", FLAG, ".facts.copies[].spread"),
    ("facts", "game object", VALUE, ".facts.objects.game[].origin", "game-exact game-renamed"),
    ("facts", "game object via", VALUE, ".facts.objects.game[].via", "install history"),
    ("facts", "origin verdict", VALUE, ".facts.origins[].verdict", "base premium custom unknown"),
    ("facts", "origin strength", VALUE, ".facts.origins[].strength", "weak strong"),
    ("facts", "origin with no evidence that decides", UNSET, ".facts.origins[].strength"),
    ("facts", "origin own mesh", VALUE, ".facts.origins[].own_mesh", "true false"),
    ("facts", "origin evidence", VALUE, ".facts.origins[].evidence[].aspect", "resolver graph skin-name mesh skeleton texture audio-tag graph-closeness resolver-closeness objects files"),
    ("facts", "origin evidence weight", VALUE, ".facts.origins[].evidence[].weight", "weak strong"),
    ("facts", "rig graph from", VALUE, ".facts.rigs[].graph", "archive game"),
    ("facts", "rig mesh source", VALUE, ".facts.rigs[].mesh.source", RIG_SOURCE),
    ("facts", "rig mesh own", VALUE, ".facts.rigs[].mesh.own", "true false"),
    ("facts", "rig mesh at a moved path", SET, ".facts.rigs[].mesh.moved"),
    ("facts", "rig submesh", VALUE, ".facts.rigs[].mesh.submeshes[].visibility", VISIBILITY),
    ("facts", "rig skeleton source", VALUE, ".facts.rigs[].skeleton.source", RIG_SOURCE),
    ("facts", "rig skeleton own", VALUE, ".facts.rigs[].skeleton.own", "true false"),
    ("facts", "rig skeleton at a moved path", SET, ".facts.rigs[].skeleton.moved"),
    ("facts", "rig clip source", VALUE, ".facts.rigs[].clips[].source", RIG_SOURCE),
    ("facts", "rig clip own", VALUE, ".facts.rigs[].clips[].own", "true false"),
    ("facts", "rig clip at a moved path", SET, ".facts.rigs[].clips[].moved"),
    ("facts", "rig part unreadable", VALUE, ".facts.rigs[].unreadable[]", "mesh skeleton"),
    ("facts", "clips graph found", VALUE, ".facts.clips[].at.*.graph_found", "true false"),
    ("facts", "clips on the unit's own rig", VALUE, ".facts.clips[].at.*.own_rig", "true false"),
    ("facts", "clips own, off the slot's joints", POSITIVE, ".facts.clips[].at.*.own_off"),
    ("facts", "clips unknown", POSITIVE, ".facts.clips[].at.*.unknown"),
    ("facts", "clips reused from another skin", SET, ".facts.clips[].at.*.reused"),
    ("facts", "clips reused on the same joints", VALUE, ".facts.clips[].at.*.reused[].same_joints", "true false"),
    ("facts", "slot the game has a bin at", VALUE, ".facts.candidates[].slots.*.game_has_bin", "true false"),
    ("facts", "check aspect", VALUE, ".facts.candidates[].slots.*.checks[].aspect", "slot mesh submeshes skeleton rig clips"),
    ("facts", "check fit", VALUE, ".facts.candidates[].slots.*.checks[].fit", "fits fits-with-fix does-not-fit unknown"),
    ("facts", "check why", VALUE, ".facts.candidates[].slots.*.checks[].why.kind", CHECK_WHY),
    ("facts", "check aspect", VALUE, ".targets.decisions[].candidates[].checks[].aspect"),
    ("facts", "check fit", VALUE, ".targets.decisions[].candidates[].checks[].fit"),
    ("facts", "check why", VALUE, ".targets.decisions[].candidates[].checks[].why.kind"),
    ("facts", "clips blocked", VALUE, ".targets.decisions[].candidates[].checks[].why.blocks[].kind", "refused needs-own-rig own-off"),
    ("facts", "rework made", VALUE, ".facts.reworks[].made", "before after unknown"),
    ("facts", "rework", VALUE, ".facts.reworks[].rework"),
    ("decision", "decision reason", VALUE, ".targets.decisions[].reason", REASONS),
    ("decision", "decision with no target", UNSET, ".targets.decisions[].chosen"),
    ("decision", "decision unfit", FLAG, ".targets.decisions[].unfit"),
    ("decision", "decision origin on the base's rig", FLAG, ".targets.decisions[].base_rig_origin"),
    ("decision", "decision with chroma followers", SET, ".targets.decisions[].chroma_followers"),
    ("decision", "decision taken by another unit", SET, ".targets.decisions[].taken_by"),
    ("decision", "decision by the clips played", SET, ".targets.decisions[].played"),
    ("decision", "decision makes a new bin", SET, ".targets.decisions[].new_bin"),
    ("decision", "decision made before a rework", SET, ".targets.decisions[].made_before"),
    ("decision", "candidate", VALUE, ".targets.decisions[].candidates[].why", "skin0 first|the premium origin|the skin whose clips it plays|the skin whose skeleton it takes|the champion's target|the policy"),
    ("decision", "candidate feasible", VALUE, ".targets.decisions[].candidates[].feasible", "true false"),
    ("decision", "alternative reason", VALUE, ".targets.decisions[].alternatives[].reason"),
    ("decision", "targets conflict", SET, ".targets.conflicts"),
    ("plan", "unit left", VALUE, ".port.shipping.left.*.why", LEFT),
    ("plan", "file verdict", VALUE, ".port.refs.units.*.*.verdict", "game own unmounted dangling"),
    ("plan", "object reached as", VALUE, ".port.reach.units.*.objects[][]", "entry resolver graph game carried", fate),
    ("plan", "reach carries game objects for overrides", POSITIVE, ".port.reach.units.*.carried_for_overrides"),
    ("plan", "reach enters the champion's objects", SET, ".port.reach.units.*.champion"),
    ("plan", "reach keeps links to the game's objects", SET, ".port.reach.units.*.game_links"),
    ("plan", "unit ships its graph", SET, ".port.units[].graph"),
    ("plan", "unit without a resolver", UNSET, ".port.units[].resolver"),
    ("plan", "renames retarget a unit", SET, ".port.renames.retargeted"),
    ("plan", "renames to the target's names", SET, ".port.renames.to_target"),
    ("plan", "renames swap game names", SET, ".port.renames.game_swaps"),
    ("plan", "renames keep links to the source slot", POSITIVE, ".port.renames.links_kept"),
    ("plan", "references pointed at the game's files", POSITIVE, ".port.repointed"),
    ("plan", "hud entry with an unspelled field", SET, ".port.hud.champions[].unspelled"),
    ("audio", "bank role", VALUE, ".facts.sounds[].units[].files[].role", "media events stub package"),
    ("audio", "bank in a locale's WAD", SET, ".facts.sounds[].units[].files[].locale"),
    ("audio", "bank unreadable", SET, ".facts.sounds[].units[].files[].unreadable"),
    ("audio", "bank with clips of the mod's own", POSITIVE, ".facts.sounds[].units[].files[].clips.own"),
    ("audio", "sound unit voice", VALUE, ".facts.sounds[].units[].voice", "true false"),
    ("audio", "carried unit layer", VALUE, ".port.audio.units[].units[].layer", "base"),
    ("audio", "carried unit layer=voice", UNSET, ".port.audio.units[].units[].layer.voice"),
    ("audio", "carried unit layer=voice-locale", SET, ".port.audio.units[].units[].layer.voice"),
    ("audio", "carried unit with events of its own", SET, ".port.audio.units[].units[].tag"),
    ("audio", "carried unit over the game's events", UNSET, ".port.audio.units[].units[].tag"),
    ("audio", "carried unit stands in for a game unit", SET, ".port.audio.units[].units[].replaces"),
    ("audio", "sound literals renamed", SET, ".port.audio.units[].renamed"),
    ("package", "chunk origin", VALUE, ".package.chunks[].origin", "kept delta whole contained made bank"),
    ("package", "layer", VALUE, ".package.chunks[].layer", "base hud voice voice-locale", layer),
    ("package", "patch of", VALUE, ".package.patches[].of", "skin root"),
    ("package", "patch layer", VALUE, ".package.patches[].layer", "base hud voice voice-locale", layer),
    ("package", "patch adds links", SET, ".package.patches[].links"),
    ("package", "patch not applied", UNSET, ".package.patches[].applied"),
    ("package", "patch outcome", VALUE, ".package.patches[].applied.outcome", "applied failed no-game-bin"),
    ("package", "patch with skipped edits", SET, ".package.patches[].applied.skipped"),
    ("package", "again built", VALUE, ".again.built", "true false"),
    ("package", "again redated", SET, ".again.redated"),
    ("package", "again changes values", SET, ".again.fixups"),
    ("package", "again changes values by", VALUE, ".again.fixups[].fixup"),
    ("package", "again changes chunks", POSITIVE, ".again.chunks_differing"),
]


def data(code, field, how, known=None, name=None):
    """A rule on a field of a diagnostic's data. The case is named by the code and the field."""
    path = f"diagnostics[{code}].data.{field}"
    leaf = field.replace("[]", "").split(".")[-1]
    return (CODE_AREAS[code.split(".")[0]], name or f"{code} {leaf}", how, path, known)


RULES += [
    data("archive.budget-exceeded", "limit", VALUE, "archive-file inflated"),
    data("archive.wad-unreadable", "why", VALUE, "version table-larger-than-data too-many-chunks mount chunk-too-large chunk-past-end chunk-unreadable panicked"),
    data("file.format-converted", "files[].how", VALUE, CONVERSIONS),
    data("file.format-unreadable", "files[].why", VALUE, "count-larger-than-data read write panicked"),
    data("file.moved", "files[].how", VALUE, CONVERSIONS),
    data("file.moved", "files[].rename", VALUE, "variant-tag resolution-prefix-2x resolution-prefix-4x variant-tag-and-prefix-2x variant-tag-and-prefix-4x base-suffix shared-bin"),
    data("file.moved", "passed_over", SET, name="file.moved passes a copy over"),
    data("file.older-game", "still_shipped", POSITIVE, name="file.older-game of bytes the game still ships"),
    data("bin.link-missing", "links[].former", VALUE, "true false"),
    data("bin.taken-from-game", "history", VALUE, "true false"),
    data("bin.taken-from-game", "former", SET, name="bin.taken-from-game from a former bin"),
    data("bin.taken-from-game", "unclaimed", SET, name="bin.taken-from-game with files no bin names"),
    data("bin.taken-from-game", "stale", POSITIVE, name="bin.taken-from-game with stale files"),
    data("bin.taken-from-game", "never", POSITIVE, name="bin.taken-from-game with files the game never held"),
    data("rig.submesh-copied", "submeshes[].visibility", VALUE, VISIBILITY),
    data("rig.unreadable", "parts[]", VALUE, "mesh skeleton"),
    data("target.chosen", "reason", VALUE, "base paste-cleaned slot fallback clips-origin skeleton-origin sub-follows"),
    data("target.forced-unfit", "follows", VALUE, "true false"),
    data("check.other-skin", "values[].value", VALUE, "skin-name audio-tag bank-unit skeleton simple-skin texture"),
    data("unit.left", "units[].why", VALUE, LEFT),
    data("unit.left-with-work", "units[].kind", VALUE, "slot-work unsupported"),
    data("unit.ported", "graph", VALUE, "true false"),
    data("unit.ported", "new_bin", VALUE, "true false"),
    data("ref.dangling", "former", POSITIVE, name="ref.dangling at a former path"),
    data("hud.carried", "unspelled", POSITIVE, name="hud.carried with an unspelled field"),
    data("audio.carried", "banks[].carry.shape", VALUE, "overlay own"),
    data("audio.carried", "banks[].carry.from_mod", VALUE, "true false"),
    data("audio.carried", "banks[].carry.shared", POSITIVE, name="audio.carried events sharing an id with the game's"),
    data("audio.carried", "banks[].carry.silent", POSITIVE, name="audio.carried sounds that play nothing"),
    data("audio.carried", "banks[].carry.unnamed", SET, name="audio.carried events no unit's list names"),
    data("audio.carried", "banks[].carry.successor", SET, name="audio.carried events from the game's bank at an older one's path"),
    data("audio.carried", "banks[].layer", VALUE, "base"),
    data("audio.carried", "banks[].layer.voice", UNSET, name="audio.carried layer=voice"),
    data("audio.carried", "banks[].layer.voice", SET, name="audio.carried layer=voice-locale"),
    data("audio.carried", "banks[].replaces", SET, name="audio.carried stands in for a game unit"),
    data("audio.carried", "once", SET, name="audio.carried a locale copy once"),
    data("audio.not-carried", "left[].why", VALUE, "no-game-audio locale-differs no-events-bank no-event no-unit-to-model new-bin"),
    data("audio.literals-renamed", "to_mod", POSITIVE, name="audio.literals-renamed to the mod's events"),
    data("audio.literals-renamed", "to_base", POSITIVE, name="audio.literals-renamed to the base's"),
    data("package.texture-contained", "contained", POSITIVE, name="package.texture-contained contained"),
    data("package.texture-contained", "kept", KEYS, TEX_REFUSALS),
    data("package.ported", "unused", POSITIVE, name="package.ported left=unused"),
    data("package.ported", "former_path", POSITIVE, name="package.ported left=former-path"),
    data("package.ported", "stale", POSITIVE, name="package.ported left=stale"),
    data("package.ported", "orphaned", POSITIVE, name="package.ported left=orphaned"),
    data("package.ported", "game", POSITIVE, name="package.ported left=game"),
    data("package.ported", "not_carried", POSITIVE, name="package.ported left=not-carried"),
    data("package.ported", "recut", POSITIVE, name="package.ported left=recut"),
    data("package.in-place", "unused", POSITIVE, name="package.in-place left=unused"),
    data("package.in-place", "former_path", POSITIVE, name="package.in-place left=former-path"),
    data("package.in-place", "stale", POSITIVE, name="package.in-place left=stale"),
    data("package.in-place", "orphaned", POSITIVE, name="package.in-place left=orphaned"),
    data("package.in-place", "made", POSITIVE, name="package.in-place ships files a fixup made"),
    data("package.in-place", "rewritten", POSITIVE, name="package.in-place writes bins again"),
    data("package.in-place", "whole", POSITIVE, name="package.in-place writes a bin whole"),
    data("package.in-place", "unnamed", POSITIVE, name="package.in-place keeps files with no path spelled"),
    data("check.done", "check", VALUE, "round-trip types apply base-skin"),
    data("check.done", "against.source", VALUE, "schema install"),
    data("check.type-mismatch", "fields[].found.kind", VALUE, name="check.type-mismatch found"),
    data("check.type-mismatch", "fields[].wants.kind", VALUE, name="check.type-mismatch wants"),
    data("check.round-trip-failed", "problem", VALUE, "unreadable entries object"),
    data("package.thumbnail-dropped", "why", VALUE, "entry-too-large entry-unreadable not-an-image not-encoded too-large"),
]

RULES += [
    ("audio", "unit leaves out a game unit", SET, ".port.audio.units[].left_out"),
]

# Parts of a report that hold no case: what the run was given, what another part says again,
# and maps keyed by names. Nothing under them is read.
SKIP = {
    ".mod", ".inputs", ".schema", ".halted", ".changes", ".diagnostics",
    ".facts.candidates[].origin", ".facts.candidates[].slots.*.clips", ".facts.copies[].copies",
    ".facts.clips[].at.*.reused[].extra_joints", ".facts.rigs[].skeleton.joints",
    ".targets.policy", ".targets.decisions[].policy", ".targets.decisions[].origin", ".targets.decisions[].placed",
    ".targets.decisions[].alternatives[].policy", ".targets.decisions[].made_before", ".targets.conflicts",
    ".port.options", ".port.layout", ".port.files", ".port.renames.names", ".port.audio.banks", ".port.audio.units[].renamed",
    ".port.hud.champions[].edits", ".port.hud.champions[].files",
    ".package.paths", ".package.entries", ".package.chunks[].objects", ".package.chunks[].values", ".package.patches[].applied.objects",
    ".again.chunks", ".again.redated", ".evidence",
    "diagnostics[inventory.executable].data", "diagnostics[inventory.bin-text].data", "diagnostics[inventory.source].data",
    "diagnostics[inventory.nested-archive].data", "diagnostics[inventory.os-artifact].data", "diagnostics[inventory.unknown].data",
}  # fmt: skip

# Fields that hold a name, an id, a hash, a path or a sentence wherever they are.
FREE = set(
    "id name spelled wad unit units champion character character_name hash build detail of entry entries first last "
    "object objects file files reference tags tag events path paths content shipped place version bin bins loads "
    "locale into taken_by target dir root namespace banks carried dangling leftovers game_links links skipped to "
    "text example rework patch class field fields copies unclaimed passed_over resolver names edited "
    "packed_locale root_bin record records ships material shader thumbnail".split()
)
# Places that hold free text under a field name that is a closed set elsewhere.
OPEN = {
    ".files[].from", ".facts.rigs[].clips[].moved", ".facts.rigs[].mesh.moved", ".facts.rigs[].skeleton.moved",
    ".facts.candidates[].base_rig_origin", ".facts.candidates[].spread", ".facts.copies[].copies[].kind",
    ".port.shipping.left.*.by", ".port.units[].graph", ".port.audio.units[].units[].voice", ".provenance[].vfx",
    ".port.audio.units[].left_out[]",
    ".port.reach.units.*.game[]", ".port.reach.units.*.champion[]",
    ".port.renames.game_swaps[][]", ".port.renames.to_target[][]",
    ".targets.decisions[].candidates[].checks[].why.level", ".targets.decisions[].candidates[].checks[].why.graph_found",
    ".targets.decisions[].candidates[].checks[].why.blocks[].own_rig",
    "diagnostics[bin.unreadable].data.why", "diagnostics[audio.carried].data.banks[].source",
    "diagnostics[audio.not-carried].data.left[].source", "diagnostics[target.superseded].data.by",
    "diagnostics[unit.left].data.units[].by", "diagnostics[file.format-converted].data.files[].from",
    "diagnostics[file.format-converted].data.files[].made", "diagnostics[file.moved].data.files[].from",
    "diagnostics[file.moved].data.files[].made", "diagnostics[bin.taken-from-game].data.former[]",
    "diagnostics[wad.duplicate-differs].data.carried", "diagnostics[audio.carried].data.banks[].carry.tag",
    "diagnostics[check.type-mismatch].data.fields[].found.item", "diagnostics[check.type-mismatch].data.fields[].wants.item",
    "diagnostics[archive.info-unreadable].data.reason", "diagnostics[audio.bank-unreadable].data.banks[].reason",
    "diagnostics[audio.carried].data.once[].source", "diagnostics[wad.several-champions].data.champions[]",
}  # fmt: skip

DYNAMIC = re.compile(r"^(%[a-z]+\d+|[0-9a-f]{8}|[0-9a-f]{16}|[0-9a-f]{64}|\d+)$")
AT = collections.defaultdict(list)
for rule in RULES:
    AT[rule[3]].append((rule[2], rule[0], rule[1], rule[5] if len(rule) > 5 else None))


def text(value):
    """A value as a case spells it: a flag as `true` or `false`."""
    return str(value).lower() if isinstance(value, bool) else str(value)


class Found:
    """The cases of one report, each with how often it occurs, and the places not known."""

    def __init__(self):
        self.cases = collections.Counter()
        self.unknown = {}

    def add(self, area, case, times=1):
        self.cases[(area, case)] += times

    def node(self, value, path):
        """Applies the rules at a path to the value there, then reads what the value holds."""
        rules = AT.get(path)
        for how, area, name, fold in rules or ():
            self.rule(how, area, name, fold, value)
        if path in SKIP:
            return
        if isinstance(value, dict):
            if any(how == KEYS for how, *_ in rules or ()):
                return
            for key, item in value.items():
                self.node(item, path + (".*" if DYNAMIC.match(key) else "." + key))
        elif isinstance(value, list):
            for item in value:
                self.node(item, path + "[]")
        elif not rules and isinstance(value, (str, bool)) and path not in OPEN:
            leaf = path.replace("[]", "").rsplit(".", 1)[-1]
            if leaf not in FREE:
                self.unknown.setdefault(path, text(value))

    def rule(self, how, area, name, fold, value):
        if how == VALUE:
            if isinstance(value, (str, bool)):
                value = fold(value) if fold else value
                if value is not None:
                    self.add(area, f"{name}={text(value)}")
        elif how == FLAG:
            if value is True:
                self.add(area, name)
        elif how == SET:
            if value is not None and value != [] and value != {}:
                self.add(area, name)
        elif how == UNSET:
            if value is None:
                self.add(area, name)
        elif how == POSITIVE:
            if isinstance(value, (int, float)) and value > 0:
                self.add(area, name)
        elif how == KEYS:
            for key, item in (value or {}).items():
                if item:
                    self.add(area, f"{name}={key}")

    def changes(self, rows):
        """What the fixups changed: each fixup, each kind of change it made, and each pair
        of kinds a value went between."""
        seen = collections.Counter()
        for row in rows or ():
            fixup = row["fixup"]
            self.add("fixups", f"fixup {fixup}")
            for change in row["changes"]:
                before, after = change.get("before"), change["after"]
                kinds = None
                if after["as"] == "value":
                    kinds = f"{before['kind'] if before else 'nothing'}>{after['kind']}"
                elif after["as"] == "unconvertible":
                    kinds = f"{before['kind']} wants {after['wants']}"
                seen[(fixup, after["as"], kinds, change["reached"])] += 1
        for (fixup, how, kinds, reached), times in seen.items():
            self.add("fixups", f"fixup {fixup} as={how}", times)
            if kinds:
                self.add("fixups", f"fixup {fixup} {kinds}", times)
            if not reached:
                self.add("fixups", "change in an object no skin entry reaches", times)

    def diagnostics(self, report):
        """Each code, the fixup or the check that raised it, the record it rests on, and the
        closed sets of its data."""
        for row in report.get("diagnostics") or ():
            code = row["code"]
            area = CODE_AREAS.get(code.split(".")[0], "run")
            self.add(area, code)
            if row.get("by"):
                self.add(area, f"{code} by={row['by']}")
            if row["id"] == report.get("halted"):
                self.add("run", f"halt {code}")
            for record in records(row):
                self.add("records", f"record {record}")
            self.node(row.get("data"), f"diagnostics[{code}].data")


def records(row):
    """The records a diagnostic rests on: its own, and those its data names."""
    found = {row["record"]} if row.get("record") else set()
    stack = [row.get("data")]
    while stack:
        value = stack.pop()
        if isinstance(value, dict):
            for key, item in value.items():
                if key == "record" and isinstance(item, str):
                    found.add(item)
                elif key == "records" and isinstance(item, list):
                    found.update(item)
                else:
                    stack.append(item)
        elif isinstance(value, list):
            stack.extend(value)
    return sorted(found)


def cases_of(report):
    """The cases one report shows."""
    found = Found()
    found.node(report, "")
    found.changes(report.get("changes"))
    found.diagnostics(report)
    return found


def of_file(item):
    run, sha = item
    found = cases_of(corpus.read(run, sha))
    return sha, dict(found.cases), found.unknown


def declared(codes):
    """The cases the program declares: the diagnostic codes of the codes table, and the
    values of the closed sets of `RULES`. A value with a space in it is separated by `|`."""
    known = {}
    with open(codes, encoding="utf-8") as f:
        for line in f:
            cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
            if len(cells) >= 3 and re.fullmatch(r"`[a-z0-9-]+\.[a-z0-9-]+`", cells[0]) and cells[1] in ("note", "warning", "error", "fatal"):
                code = cells[0].strip("`")
                known[(CODE_AREAS.get(code.split(".")[0], "run"), code)] = None
    for area, name, how, _path, *rest in RULES:
        values = rest[0] if rest else None
        if how not in (VALUE, KEYS):
            known[(area, name)] = None
        elif values:
            for value in values.split("|") if "|" in values else values.split():
                known[(area, f"{name}={value}")] = None
    for fixup in FIXUPS.split():
        known[("fixups", f"fixup {fixup}")] = None
    return known


def gather(runs, jobs):
    """Every case of the runs: per case and run the mods showing it and the occurrences, and
    per mod its cases. The occurrences of a case in a mod are those of the run showing most."""
    mods = collections.defaultdict(dict)
    shown_by = collections.defaultdict(set)
    unknown = {}
    for name, run in runs:
        items = [(run, sha) for sha in corpus.reports(run)]
        for sha, cases, places in corpus.over(of_file, items, jobs):
            for case, times in cases.items():
                mods[sha][case] = max(times, mods[sha].get(case, 0))
                shown_by[case].add(name)
            for path, example in places.items():
                unknown.setdefault(path, example)
        print(f"{name}: {len(items)} reports", flush=True)
    return mods, shown_by, unknown


def table(mods, shown_by):
    """One row per case: area, case, mods, occurrences, runs, and up to five examples."""
    holders = collections.defaultdict(list)
    for sha, cases in mods.items():
        for case in cases:
            holders[case].append(sha)
    rows = []
    for (area, case), shas in holders.items():
        shas.sort(key=lambda sha: (len(mods[sha]), sha))
        times = sum(mods[sha][(area, case)] for sha in shas)
        rows.append((area, case, len(shas), times, ",".join(sorted(shown_by[(area, case)])), shas[:5]))
    rows.sort(key=lambda row: (AREAS.index(row[0]), row[1]))
    return rows


def document(path, rows, unseen, mods, runs, dated):
    """Writes the generated tables of a document again, between its marker lines."""
    names = " and ".join(f"`{name}`" for name, _ in runs)
    cases = [f"{len(rows)} cases in {len(mods):,} mods, from the runs {names} of {dated}.", ""]
    for area in AREAS:
        mine = [row for row in rows if row[0] == area]
        if not mine:
            continue
        cases += [f"### {area}", "", "| case | mods | example |", "|---|---:|---|"]
        cases += [f"| `{case}` | {count:,} | `{examples[0][:12]}` |" for _, case, count, _, _, examples in mine]
        cases.append("")
    missing = [f"{len(unseen)} cases, against the runs {names} of {dated}.", ""]
    for area in AREAS:
        mine = [case for a, case in unseen if a == area]
        if mine:
            missing += [f"### {area}, not seen", ""] + [f"- `{case}`" for case in mine] + [""]
    with open(path, encoding="utf-8") as f:
        body = f.read()
    for marker, lines in (("cases", cases), ("unseen", missing)):
        block = re.compile(rf"(<!-- cases\.py: {marker} -->\n).*?(<!-- cases\.py: end -->)", re.S)
        if not block.search(body):
            sys.exit(f"{path} has no `<!-- cases.py: {marker} -->` block")
        body = block.sub(lambda m: m.group(1) + "\n" + "\n".join(lines).rstrip("\n") + "\n\n" + m.group(2), body)
    corpus.write(path, body)


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("runs", nargs="+", metavar="NAME=RUN")
    ap.add_argument("--out", required=True)
    ap.add_argument("--codes", default=os.path.join(corpus.ROOT, "docs", "design", "CODES.md"))
    ap.add_argument("--doc")
    ap.add_argument("--dated")
    corpus.add_jobs(ap)
    args = ap.parse_args()
    if args.doc and not args.dated:
        ap.error("--doc needs --dated, the date of the runs")
    runs = [tuple(run.split("=", 1)) for run in args.runs]
    os.makedirs(args.out, exist_ok=True)

    mods, shown_by, unknown = gather(runs, args.jobs)
    rows = table(mods, shown_by)
    known = declared(args.codes)
    seen = {(area, case) for area, case, *_ in rows}
    unseen = sorted((key for key in known if key not in seen), key=lambda key: (AREAS.index(key[0]), key[1]))
    undeclared = sorted(case for area, case in seen if undeclared_value(area, case, known))

    lines = ["area\tcase\tmods\toccurrences\truns\texamples"]
    lines += [f"{area}\t{case}\t{count}\t{times}\t{names}\t{','.join(examples)}" for area, case, count, times, names, examples in rows]
    corpus.write(os.path.join(args.out, "cases.tsv"), "\n".join(lines) + "\n")
    corpus.write(os.path.join(args.out, "unseen.tsv"), "area\tcase\n" + "".join(f"{area}\t{case}\n" for area, case in unseen))
    notes = [f"place not in the table: {path}  (for example {example!r})" for path, example in sorted(unknown.items())]
    notes += [f"value not declared in the table: {case}" for case in undeclared]
    corpus.write(os.path.join(args.out, "unknown.txt"), "".join(note + "\n" for note in notes))
    index = {key: at for at, key in enumerate((area, case) for area, case, *_ in rows)}
    listing = {
        "runs": [name for name, _ in runs],
        "cases": [[area, case] for area, case, *_ in rows],
        "mods": {sha: sorted(index[case] for case in mods[sha]) for sha in sorted(mods)},
    }
    corpus.write(os.path.join(args.out, "mods.json"), json.dumps(listing, separators=(",", ":")) + "\n")
    if args.doc:
        document(args.doc, rows, unseen, mods, runs, args.dated)

    by_area = collections.Counter(area for area, *_ in rows)
    print(f"{len(mods)} mods, {len(rows)} cases seen, {len(unseen)} declared and not seen")
    for area in AREAS:
        print(f"  {area:10} {by_area[area]:5} seen {sum(1 for a, _ in unseen if a == area):5} not seen")
    for note in notes:
        print(note)


def undeclared_value(area, case, known):
    """Whether a case is a value of a closed set whose declared values do not hold it."""
    name = case.split("=", 1)[0]
    if "=" not in case:
        return "." in case.split(" ")[0] and " " not in case and (area, case) not in known
    declares = any(key[0] == area and key[1].startswith(name + "=") for key in known)
    return declares and (area, case) not in known


if __name__ == "__main__":
    main()
