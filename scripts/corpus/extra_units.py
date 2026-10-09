"""The champion units a port ships beside each champion's main one, sorted into leftovers and
multi-skin mods by what the reports of a run already hold (#ADR-0020). No archive is opened.

extra_units.py RUN --out DIR [--list FILE | --reduced] [--builds FILE] [--seed N] [--check SHA256] [--jobs N]
extra_units.py --rows FILE --out DIR [--seed N] [--check SHA256]

A champion whose port ships more than one unit has a main unit: the one at skin 0, else the
one with the most own work. Every other unit it ships is an extra unit, one row each, with:

  - its slot, target, reason and origin, and the main unit's;
  - its bin's bytes: the mod's own, the install's or an older build's, and those builds;
  - the objects it reaches: the game's of its slot, of another slot, of no slot, and own
    (unknown to the history), and how many own objects the main unit writes too, by bytes;
  - the files it names by verdict, those the mod ships split into own bytes and the game's,
    and how many of each the main unit ships too, by bytes;
  - of the files the main unit does not ship: the own ones by kind and place, and the models,
    skeletons and textures in its slot's folder;
  - its own clips: in a bank of its own slot or of the base's sound unit, and how many sit in
    a bank the main unit names too, by path or by bytes.

A mod with extra units is a multi-skin mod when an extra unit ships an own model, skeleton or
texture the main unit does not, a leftover when no extra unit holds own work the main unit
does not carry, and ambiguous otherwise. Own clips in a bank of the base's sound unit are not
an extra unit's own work: the game's skin bins name the base's units, so a leftover names the
mod's base audio as every skin does.

A family is the archives that share more than half of the smaller one's own files, by bytes:
the versions and the series of one work count once.

Inputs:

  RUN       a run directory of `run.py port`
  --list    the SHA-256s to read, one per line, or a reduced corpus (default: every report
            of RUN)
  --reduced the reduced corpus beside this script
  --builds  builds.tsv of the history index the run read, to name a bin's builds (default:
            REFORGE_INDEX's, else builds are named by their positions)
  --rows    rows.jsonl of an earlier scan, read instead of RUN, with reports.jsonl beside it
  --seed    the seed the spot check draws ten families of each bucket with (default 20261009)
  --check   a mod whose bucket the user settled: listed first, its family not drawn

Outputs, in DIR:

  rows.jsonl     one extra unit per line, every field
  reports.jsonl  one report per line: what else the scan counts of it, null for no port
  units.tsv      the extra units, one line each
  mods.tsv       one mod with extra units per line: its bucket, its family and why
  spot.tsv       the checks, then ten families of each bucket drawn by the seed
  signals.tsv    per signal, the units and mods of the certain buckets it alone misplaces
  summary.txt    the counts"""
import argparse, collections, json, os, random, re, sys, unicodedata

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import corpus  # noqa: E402

MODEL, SKELETON, ANIMATION, TEXTURE = "model", "skeleton", "animation", "texture"
KINDS = {"skn": MODEL, "skl": SKELETON, "anm": ANIMATION, "tex": TEXTURE, "dds": TEXTURE}
# The work that makes a unit the author's for its slot (the plan's multi-skin test).
SLOT_WORK = (MODEL, SKELETON, TEXTURE)
GAME_BYTES = ("install", "history")
BASE_UNIT = re.compile(r"_base_", re.I)
LEFTOVER, MULTI, AMBIGUOUS = "leftover", "multi-skin", "ambiguous"
BUCKETS = [LEFTOVER, MULTI, AMBIGUOUS]


def clean(text, width=60):
    """A mod's text as a terminal or a table may show it: no control or format character,
    one line, at most `width` characters."""
    text = "".join("?" if unicodedata.category(c)[0] == "C" else c for c in str(text or ""))
    text = " ".join(text.split())
    return text if len(text) <= width else text[: width - 3] + "..."


def build_names(path):
    """The builds of the history index by position."""
    names = {}
    if path and os.path.exists(path):
        with open(path, encoding="utf-8") as f:
            for line in f:
                cells = line.rstrip("\n").split("\t")
                if len(cells) > 2 and cells[0].isdigit():
                    names[int(cells[0])] = cells[2]
    return names


class Report:
    """One report's tables, by id."""

    def __init__(self, report):
        self.r = report
        facts = report.get("facts") or {}
        port = report.get("port") or {}
        self.files = {f["id"]: f for f in report.get("files") or []}
        self.prov = {p["file"]: p for p in report.get("provenance") or []}
        self.units = {u["id"]: u for u in report.get("units") or []}
        self.characters = {c["id"]: c for c in report.get("characters") or []}
        self.decisions = {d["unit"]: d for d in (report.get("targets") or {}).get("decisions") or []}
        self.game = {g["object"]: g for g in (facts.get("objects") or {}).get("game") or []}
        self.sounds = {s["unit"]: s for s in facts.get("sounds") or []}
        self.rigs = {r["unit"]: r for r in facts.get("rigs") or []}
        self.shipping = port.get("shipping") or {}
        self.reach = (port.get("reach") or {}).get("units") or {}
        self.refs = (port.get("refs") or {}).get("units") or {}
        self.dirs = {u["unit"]: u["dir"] for u in port.get("units") or [] if u.get("dir")}
        self.names = (port.get("renames") or {}).get("names") or {}
        chunks = [c for c in (report.get("package") or {}).get("chunks") or [] if c.get("objects")]
        self.chunks = collections.defaultdict(dict)
        for chunk in chunks:
            for unit, where in self.dirs.items():
                if chunk["path"].startswith(where + "/"):
                    self.chunks[unit].update(chunk["objects"])

    def bytes_of(self, file):
        """Where a file's bytes come from: `own`, `install` or `history`."""
        return (self.prov.get(file) or {}).get("origin") or "own"

    def shipped(self, unit):
        """The files a unit names that the mod ships, by id."""
        return sorted({v["file"] for v in (self.refs.get(unit) or {}).values() if v["verdict"] == "own"})

    def banks(self, unit):
        """The bank files a unit's audio names, each once: sound unit, path, file, own clips."""
        found = {}
        for sound in (self.sounds.get(unit) or {}).get("units") or []:
            for bank in sound["files"]:
                found.setdefault((bank["path"], bank["file"]), (sound["name"], bank["clips"]["own"]))
        return [(name, path, file, own) for (path, file), (name, own) in sorted(found.items())]

    def rig_files(self, unit):
        """The mesh and the skeleton a unit's rig takes from the mod, by kind."""
        rig = self.rigs.get(unit) or {}
        found = {}
        for part, kind in (("mesh", MODEL), ("skeleton", SKELETON)):
            if (rig.get(part) or {}).get("file"):
                found[rig[part]["file"]] = kind
        return found

    def own_work(self, unit):
        """How much own work a unit holds: own objects, own files and own clips."""
        objects = sum(1 for o, fate in (self.reach.get(unit) or {}).get("objects") or [] if fate != "game" and o not in self.game)
        files = sum(1 for f in self.shipped(unit) if self.bytes_of(f) == "own")
        return objects + files + sum(own for *_, own in self.banks(unit))


def main_unit(rep, shipped):
    """The main unit of a champion's shipped units, and how it was chosen."""
    at_base = [u for u in shipped if (rep.decisions.get(u) or {}).get("chosen") == 0]
    if at_base:
        return at_base[0], "skin0"
    ranked = sorted(shipped, key=lambda u: (-rep.own_work(u), (rep.decisions.get(u) or {}).get("chosen") or 0, u))
    return ranked[0], "most-own-work"


def row_of(rep, unit, main, chosen_by, builds):
    """One extra unit's row."""
    u, m = rep.units[unit], rep.units[main]
    d, dm = rep.decisions.get(unit) or {}, rep.decisions.get(main) or {}
    target, character = d.get("chosen"), u["character_name"]
    origin, main_origin = d.get("origin") or {}, dm.get("origin") or {}
    slots = {u["slot"], target}
    bin_fact = rep.prov.get(u["file"]) or {}
    life = bin_fact.get("life") or {}

    row = {
        "sha256": rep.r["mod"]["sha256"],
        "mod": clean(f"{rep.r['mod'].get('name') or ''} {rep.r['mod'].get('version') or ''}"),
        "character": character,
        "unit": unit,
        "slot": u["slot"],
        "target": target,
        "reason": d.get("reason"),
        "origin": origin.get("verdict"),
        "origin_slot": origin.get("slot"),
        "main": main,
        "main_by": chosen_by,
        "main_slot": m["slot"],
        "main_target": dm.get("chosen"),
        "main_reason": dm.get("reason"),
        "main_origin": main_origin.get("verdict"),
        "main_origin_slot": main_origin.get("slot"),
        "main_own_work": rep.own_work(main),
        "at_main_origin": main_origin.get("verdict") == "premium" and target == main_origin.get("slot"),
        "bin": rep.bytes_of(u["file"]),
        "bin_at_path": bin_fact.get("at_path"),
        "bin_first": builds.get(life.get("first"), life.get("first")),
        "bin_last": builds.get(life.get("last"), life.get("last")),
        "bin_still_shipped": bin_fact.get("still_shipped"),
    }

    # The objects it reaches, by whose they are.
    objects = collections.Counter()
    own_objects, own_carried = [], 0
    main_digests = set(rep.chunks.get(main, {}).values())
    for obj, fate in (rep.reach.get(unit) or {}).get("objects") or []:
        game = rep.game.get(obj)
        if fate == "game":
            objects["game_as_is"] += 1
        elif game is None:
            objects["own"] += 1
            own_objects.append((obj, fate))
        elif game.get("slot") is None:
            objects["game_no_slot"] += 1
        elif game["slot"] in slots:
            objects[f"game_slot_{game['via']}"] += 1
        else:
            objects["game_other_slot"] += 1
    for obj, fate in own_objects:
        digest = rep.chunks.get(unit, {}).get(rep.names.get(obj))
        own_carried += fate in ("carried", "graph") and digest is not None and digest in main_digests
    row.update({f"objects_{key}": objects[key] for key in ("game_slot_install", "game_slot_history", "game_other_slot", "game_no_slot", "own", "game_as_is")})
    row["objects_own_entry"] = sum(1 for _, fate in own_objects if fate in ("entry", "resolver"))
    row["objects_own_carried_by_main"] = own_carried

    # The files it names, by verdict; the ones the mod ships, by their bytes, and those the main
    # unit does not ship by kind and place.
    verdicts = collections.Counter(v["verdict"] for v in (rep.refs.get(unit) or {}).values())
    row.update({f"refs_{v}": verdicts[v] for v in ("game", "own", "unmounted", "dangling")})
    main_contents = {rep.files[f]["content"] for f in rep.shipped(main)}
    rig = rep.rig_files(unit)
    files, uncarried, places, work = (collections.Counter() for _ in range(4))
    examples = []
    for file in rep.shipped(unit):
        own = rep.bytes_of(file) == "own"
        carried = rep.files[file]["content"] in main_contents
        files[("own" if own else "game") + ("_carried" if carried else "")] += 1
        if carried:
            continue
        kind = rig.get(file) or KINDS.get(rep.files[file]["kind"])
        folder = (rep.prov.get(file) or {}).get("skin_folder") or {}
        of_slot = file in rig or (folder.get("character") == character and folder.get("slot") in slots)
        path = clean(rep.files[file]["path"] or rep.files[file]["hash"], 120)
        if own:
            uncarried[kind or "other"] += 1
            places[f"{rep.files[file]['kind']}:{place_of(path, folder, character, of_slot)}"] += 1
            examples.append(path)
        if kind in SLOT_WORK:
            work[("own" if own else "game") + ("_in_slot_folder" if of_slot else "")] += 1
    row["files_own"] = files["own"] + files["own_carried"]
    row["files_own_carried_by_main"] = files["own_carried"]
    row["files_game"] = files["game"] + files["game_carried"]
    row["files_game_carried_by_main"] = files["game_carried"]
    row.update({f"own_{kind}": uncarried[kind] for kind in (MODEL, SKELETON, ANIMATION, TEXTURE, "other")})
    row["own_work_in_slot_folder"] = work["own_in_slot_folder"]
    row["game_work_in_slot_folder"] = work["game_in_slot_folder"]
    row["own_files_uncarried_places"] = dict(sorted(places.items()))
    row["own_files_uncarried_examples"] = sorted(examples)[:3]

    # The own clips of its banks, by where the bank sits and whether the main unit names it.
    main_banks = rep.banks(main)
    main_paths = {path for _, path, _, _ in main_banks}
    main_bank_contents = {rep.files[file]["content"] for _, _, file, _ in main_banks}
    clips = collections.Counter()
    for sound, path, file, own in rep.banks(unit):
        if not own:
            continue
        place = "base" if BASE_UNIT.search(sound) else "slot"
        clips[place] += own
        if path in main_paths:
            clips[f"{place}_carried_path"] += own
        elif rep.files[file]["content"] in main_bank_contents:
            clips[f"{place}_carried_bytes"] += own
    for place in ("slot", "base"):
        row[f"clips_{place}"] = clips[place]
        row[f"clips_{place}_carried_by_main"] = clips[f"{place}_carried_path"] + clips[f"{place}_carried_bytes"]
        row[f"clips_{place}_carried_by_bytes"] = clips[f"{place}_carried_bytes"]

    rig_facts = rep.rigs.get(unit) or {}
    mesh_file = (rig_facts.get("mesh") or {}).get("file")
    row["mesh"] = (rig_facts.get("mesh") or {}).get("source")
    row["mesh_own"] = (rig_facts.get("mesh") or {}).get("own")
    row["mesh_shipped_by_main"] = mesh_file is not None and rep.files[mesh_file]["content"] in main_contents
    row["skeleton"] = (rig_facts.get("skeleton") or {}).get("source")
    row["skeleton_own"] = (rig_facts.get("skeleton") or {}).get("own")
    row.update(judged(row))
    return row


def place_of(path, folder, character, of_slot):
    """Where a file sits for a unit: its own slot's folder, another skin's of its champion,
    the champion's HUD folder, or elsewhere."""
    if of_slot:
        return "slot"
    if folder.get("character") == character:
        return "other-skin"
    if f"/{character}/hud/" in path.lower():
        return "hud"
    return "other"


def judged(row):
    """What an extra unit holds that the main unit does not: own models, skeletons and
    textures, which make it the author's work for its slot, and other own work."""
    slot_work = sum(row[f"own_{kind}"] for kind in SLOT_WORK)
    own_work = {
        "own objects": row["objects_own"] - row["objects_own_carried_by_main"],
        "own files": row[f"own_{ANIMATION}"] + row["own_other"],
        "own clips of its slot": row["clips_slot"] - row["clips_slot_carried_by_main"],
    }
    if slot_work:
        verdict = MULTI
    elif any(own_work.values()):
        verdict = AMBIGUOUS
    else:
        verdict = LEFTOVER
    return {
        "verdict": verdict,
        "uncarried": {key: value for key, value in own_work.items() if value},
        "slot_work_own": slot_work,
        "clips_base_uncarried": row["clips_base"] - row["clips_base_carried_by_main"],
    }


def of_file(item):
    """A mod's extra units, and what else the scan counts of it."""
    run, sha, builds = item
    report = corpus.read(run, sha)
    port = report.get("port")
    if report.get("halted") is not None or not port:
        return sha, None, [], []
    rep = Report(report)
    ships = set(rep.shipping.get("ships") or [])
    left = collections.Counter(why["why"] for why in (rep.shipping.get("left") or {}).values())
    rows = []
    for cid, character in sorted(rep.characters.items(), key=lambda item: item[1]["name"]):
        if character["kind"] != "champion":
            continue
        shipped = sorted((u for u in character["units"] if u in ships), key=lambda u: (rep.units[u]["slot"], u))
        if len(shipped) < 2:
            continue
        main, chosen_by = main_unit(rep, shipped)
        for unit in shipped:
            if unit != main:
                rows.append(row_of(rep, unit, main, chosen_by, builds))
    facts = {
        "name": clean(f"{report['mod'].get('name') or ''} {report['mod'].get('version') or ''}"),
        "ships": len(ships),
        "left": dict(sorted(left.items())),
    }
    own = sorted({rep.files[p["file"]]["content"][:16] for p in rep.prov.values() if p["origin"] == "own"}) if rows else []
    return sha, facts, rows, own


def families(owns):
    """The family of each mod, named by its first SHA-256: mods join one when they share more
    than half of the smaller one's own files, by bytes."""
    parent = {sha: sha for sha in owns}

    def root(sha):
        while parent[sha] != sha:
            parent[sha] = parent[parent[sha]]
            sha = parent[sha]
        return sha

    holders = collections.defaultdict(list)
    for sha in sorted(owns):
        for content in owns[sha]:
            holders[content].append(sha)
    shared = collections.Counter()
    for shas in holders.values():
        for i, a in enumerate(shas):
            for b in shas[i + 1 :]:
                shared[(a, b)] += 1
    for (a, b), n in sorted(shared.items()):
        if 2 * n > min(len(owns[a]), len(owns[b])):
            ra, rb = root(a), root(b)
            parent[max(ra, rb)] = min(ra, rb)
    return {sha: root(sha) for sha in owns}


def bucket_of(rows):
    """A mod's bucket, from its extra units, and one line on why."""
    verdicts = collections.Counter(row["verdict"] for row in rows)
    at = ", ".join(f"skin{row['target']}" for row in rows)
    if verdicts[MULTI]:
        units = [row for row in rows if row["verdict"] == MULTI]
        work = sum(row["slot_work_own"] for row in units)
        bucket = MULTI
        why = f"{len(units)} of {len(rows)} extra units ship {work} own model, skeleton or texture files the main unit does not ({at})"
    elif verdicts[AMBIGUOUS]:
        held = collections.Counter()
        for row in rows:
            held.update(row["uncarried"])
        bucket = AMBIGUOUS
        why = f"{verdicts[AMBIGUOUS]} of {len(rows)} extra units hold own work the main unit does not: " + ", ".join(f"{n} {what}" for what, n in sorted(held.items())) + f" ({at})"
    else:
        bins = collections.Counter(row["bin"] for row in rows)
        bucket = LEFTOVER
        why = f"{len(rows)} extra units hold no own work the main unit does not ({at}); bins " + ", ".join(f"{n} {b}" for b, n in sorted(bins.items()))
    base = sum(row["clips_base_uncarried"] for row in rows)
    if base:
        why += f"; {base} own clips of the base's sound unit only they name"
    return bucket, why


def own_share(row):
    reached = sum(row[f"objects_{key}"] for key in ("game_slot_install", "game_slot_history", "game_other_slot", "game_no_slot", "own"))
    return row["objects_own"] / reached if reached else 0.0


# A signal: its name, and whether it says an extra unit is a leftover.
SIGNALS = [
    ("at the main unit's origin slot", lambda r: r["at_main_origin"]),
    ("reason slot", lambda r: r["reason"] == "slot"),
    ("bin of the game's bytes", lambda r: r["bin"] in GAME_BYTES),
    ("bin of an older build's bytes", lambda r: r["bin"] == "history"),
    ("no own object", lambda r: r["objects_own"] == 0),
    ("own objects at most 2% of those reached", lambda r: own_share(r) <= 0.02),
    ("own objects at most 5% of those reached", lambda r: own_share(r) <= 0.05),
    ("own objects at most 10% of those reached", lambda r: own_share(r) <= 0.10),
    ("own objects at most 25% of those reached", lambda r: own_share(r) <= 0.25),
    ("no object of another slot", lambda r: r["objects_game_other_slot"] == 0),
    ("own objects all written by the main unit too", lambda r: r["objects_own"] == r["objects_own_carried_by_main"]),
    ("no own file", lambda r: r["files_own"] == 0),
    ("own files all shipped by the main unit too", lambda r: r["files_own"] == r["files_own_carried_by_main"]),
    ("no own clip", lambda r: r["clips_slot"] + r["clips_base"] == 0),
    ("own clips all in banks the main unit names", lambda r: r["clips_slot"] + r["clips_base"] == r["clips_slot_carried_by_main"] + r["clips_base_carried_by_main"]),
    ("own clips of its slot all in banks the main unit names", lambda r: r["clips_slot"] == r["clips_slot_carried_by_main"]),
    ("at most 5 own objects the main unit does not write", lambda r: r["objects_own"] - r["objects_own_carried_by_main"] <= 5),
    ("at most 10 own objects the main unit does not write", lambda r: r["objects_own"] - r["objects_own_carried_by_main"] <= 10),
    ("no own model, skeleton or texture the main unit does not ship", lambda r: r["slot_work_own"] == 0),
    ("no model, skeleton or texture of the game's bytes in its slot's folder", lambda r: r["game_work_in_slot_folder"] == 0),
    ("mesh and skeleton the game's, not shipped", lambda r: r["mesh"] == "game" and r["skeleton"] == "game"),
    ("subset: no own object or object of another slot, own files and clips all the main unit's",
     lambda r: r["objects_own"] == 0 and r["objects_game_other_slot"] == 0 and r["files_own"] == r["files_own_carried_by_main"]
     and r["clips_slot"] + r["clips_base"] == r["clips_slot_carried_by_main"] + r["clips_base_carried_by_main"]),
]  # fmt: skip


def scored(by_mod, buckets):
    """Per signal, the units of the certain buckets it alone misplaces: a leftover's extra unit
    it keeps, a multi-skin mod's unit with an own model, skeleton or texture it drops; and how
    many units of the ambiguous mods it drops."""
    out = []
    for name, says_leftover in SIGNALS:
        kept, kept_mods, dropped, dropped_mods, ambiguous, ambiguous_mods = 0, set(), 0, set(), 0, set()
        for sha, rows in by_mod.items():
            bucket = buckets[sha][0]
            for row in rows:
                verdict = says_leftover(row)
                if bucket == LEFTOVER and not verdict:
                    kept += 1
                    kept_mods.add(sha)
                elif bucket == MULTI and row["verdict"] == MULTI and verdict:
                    dropped += 1
                    dropped_mods.add(sha)
                elif bucket == AMBIGUOUS and verdict:
                    ambiguous += 1
                    ambiguous_mods.add(sha)
        out.append((name, kept, len(kept_mods), dropped, len(dropped_mods), ambiguous, len(ambiguous_mods)))
    return out


def cell(value):
    if isinstance(value, (dict, list)):
        return json.dumps(value, sort_keys=True, ensure_ascii=True)
    return "" if value is None else str(value)


def write_tables(out, rows, mods, seed, checks):
    by_mod = collections.defaultdict(list)
    for row in rows:
        by_mod[row["sha256"]].append(row)
    buckets = {sha: bucket_of(mod_rows) for sha, mod_rows in by_mod.items()}

    fields = list(rows[0]) if rows else []
    text = "\t".join(fields) + "\n" + "".join("\t".join(cell(row[f]) for f in fields) + "\n" for row in rows)
    corpus.write(os.path.join(out, "units.tsv"), text)

    family = {sha: mod_rows[0]["family"] for sha, mod_rows in by_mod.items()}
    members = collections.Counter(family.values())
    lines = ["sha256\tmod\tbucket\tfamily\tcharacters\textra_units\twhy"]
    for sha in sorted(by_mod):
        mod_rows = by_mod[sha]
        characters = ",".join(sorted({row["character"] for row in mod_rows}))
        lines.append("\t".join((sha, mod_rows[0]["mod"], buckets[sha][0], family[sha], characters, str(len(mod_rows)), buckets[sha][1])))
    corpus.write(os.path.join(out, "mods.tsv"), "\n".join(lines) + "\n")

    def line(label, sha):
        characters = ",".join(sorted({row["character"] for row in by_mod[sha]}))
        return "\t".join((label, buckets[sha][0], sha, by_mod[sha][0]["mod"], str(members[family[sha]]), characters, buckets[sha][1]))

    # Ten families of each bucket, each by its first mod there, so that a series of uploads of
    # one work is drawn once.
    draw = random.Random(seed)
    lines = ["drawn\tbucket\tsha256\tmod\tfamily_archives\tcharacters\twhy"]
    lines += [line("check", sha) for sha in checks if sha in by_mod]
    settled = {family[sha] for sha in checks if sha in by_mod}
    for bucket in BUCKETS:
        first = {}
        for sha in sorted(by_mod):
            if buckets[sha][0] == bucket and family[sha] not in settled:
                first.setdefault(family[sha], sha)
        drawn = draw.sample(sorted(first), min(10, len(first)))
        lines += [line(f"seed {seed}", first[f]) for f in sorted(drawn, key=lambda f: first[f])]
    corpus.write(os.path.join(out, "spot.tsv"), "\n".join(lines) + "\n")

    scores = scored(by_mod, buckets)
    lines = ["signal\tleftover_units_kept\tleftover_mods\tmulti_units_dropped\tmulti_mods\tambiguous_units_dropped\tambiguous_mods"]
    lines += ["\t".join(map(str, score)) for score in scores]
    corpus.write(os.path.join(out, "signals.tsv"), "\n".join(lines) + "\n")

    summary = summarize(rows, by_mod, buckets, mods, scores, seed)
    corpus.write(os.path.join(out, "summary.txt"), summary)
    print(summary, end="")


def summarize(rows, by_mod, buckets, mods, scores, seed):
    count = collections.Counter(buckets[sha][0] for sha in by_mod)
    units = collections.Counter(buckets[row["sha256"]][0] for row in rows)
    at_origin = collections.Counter(buckets[row["sha256"]][0] for row in rows if row["at_main_origin"])
    verdicts = collections.Counter((buckets[row["sha256"]][0], row["verdict"]) for row in rows)
    halted = sum(1 for facts in mods.values() if facts is None)
    game_skin = sum(1 for facts in mods.values() if facts and facts["left"].get("game-skin"))
    game_skin_units = sum(facts["left"].get("game-skin", 0) for facts in mods.values() if facts)
    families_in = collections.defaultdict(set)
    for row in rows:
        families_in[buckets[row["sha256"]][0]].add(row["family"])
    out = [
        f"reports: {len(mods)}, {halted} with no port",
        f"mods whose port ships extra champion units: {len(by_mod)} in {len({r['family'] for r in rows})} families, {len(rows)} extra units, {len({(r['sha256'], r['character']) for r in rows})} champions",
        f"mods whose port leaves a unit as the game's own skin: {game_skin}, {game_skin_units} units",
        f"main unit: {sum(1 for r in rows if r['main_by'] == 'skin0')} extra units beside a unit at skin 0, {sum(1 for r in rows if r['main_by'] != 'skin0')} beside the one with the most own work",
        "",
        "bucket        mods  families  units  at the main unit's origin slot",
    ]
    for bucket in BUCKETS:
        out.append(f"  {bucket:11} {count[bucket]:5} {len(families_in[bucket]):9} {units[bucket]:6} {at_origin[bucket]:6}")
    out += ["", "extra units by their own verdict, per bucket of their mod:"]
    out += [f"  {bucket:11} {verdict:11} {n:6}" for (bucket, verdict), n in sorted(verdicts.items())]
    def mods_where(bucket, test):
        return sum(1 for sha, mod_rows in by_mod.items() if buckets[sha][0] == bucket and any(test(r) for r in mod_rows))

    multi = [r for r in rows if r["verdict"] == MULTI]
    out += [
        "",
        f"leftovers whose extra units name own clips of the base's sound unit the main unit does not: {mods_where(LEFTOVER, lambda r: r['clips_base_uncarried'])}",
        f"leftovers whose extra units ship a model, skeleton or texture of the game's bytes in their slot's folder: {mods_where(LEFTOVER, lambda r: r['game_work_in_slot_folder'])}",
        f"multi-skin mods with an own model or skeleton the main unit does not ship: {mods_where(MULTI, lambda r: r['own_model'] or r['own_skeleton'])}",
        f"multi-skin mods with own textures alone: {count[MULTI] - mods_where(MULTI, lambda r: r['own_model'] or r['own_skeleton'])}",
        f"multi-skin mods with such work in the unit's slot folder or its rig: {mods_where(MULTI, lambda r: r['own_work_in_slot_folder'])}",
        f"multi-skin units: {len(multi)}, {sum(1 for r in multi if r['own_work_in_slot_folder'])} with such work in their slot folder or rig",
    ]
    ambiguous = [row for row in rows if buckets[row["sha256"]][0] == AMBIGUOUS]
    held, held_units, places, shapes = collections.Counter(), collections.Counter(), collections.Counter(), collections.Counter()
    for row in ambiguous:
        held.update(row["uncarried"])
        held_units.update(row["uncarried"].keys())
        places.update(row["own_files_uncarried_places"])
        shapes[(row["bin"] in GAME_BYTES, row["mesh_shipped_by_main"])] += 1
    out += ["", "own work the main unit does not carry, in the ambiguous mods' extra units: units, items"]
    out += [f"  {what:24} {held_units[what]:6} {n:8}" for what, n in sorted(held.items())]
    if places:
        out += ["", "those own files by kind and place (slot, other-skin, hud, other): files"]
        out += [f"  {what:24} {n:8}" for what, n in sorted(places.items(), key=lambda kv: (-kv[1], kv[0]))[:16]]
    out += ["", "the ambiguous mods' extra units by how much of each they hold: units (mods)"]
    for what in ("own objects", "own clips of its slot"):
        sizes = collections.defaultdict(set)
        for row in ambiguous:
            sizes[corpus.size_of(row["uncarried"].get(what, 0))].add((row["sha256"], row["unit"]))
        cells = [f"{name} {len(sizes[name])} ({len({sha for sha, _ in sizes[name]})})" for name in corpus.SIZES if sizes[name]]
        out.append(f"  {what:24} " + ", ".join(cells))
    out += ["", "the ambiguous mods' extra units by bin and mesh: units"]
    for (game_bin, main_mesh), n in sorted(shapes.items()):
        out.append(f"  bin {'the game' if game_bin else 'own':8} mesh {'the main unit' if main_mesh else 'its own or the game':20} {n:6}")
    empty_main = [sha for sha, mod_rows in by_mod.items() if mod_rows[0]["main_own_work"] == 0]
    out += ["", f"mods whose main unit holds no own work: {len(empty_main)}, " + ", ".join(f"{b} {sum(1 for s in empty_main if buckets[s][0] == b)}" for b in BUCKETS)]
    out += ["", f"signals, alone (spot check drawn with seed {seed}):", "  leftover units kept, mods | multi-skin units dropped, mods | ambiguous units dropped, mods"]
    out += [f"  {name[:72]:72} {a:5} {b:5} | {c:5} {d:5} | {e:5} {f:5}" for name, a, b, c, d, e, f in scores]
    return "\n".join(out) + "\n"


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("run", nargs="?")
    ap.add_argument("--out", required=True)
    ap.add_argument("--list", help="a text file of SHA-256s, or a reduced corpus")
    ap.add_argument("--reduced", action="store_true", help="the reduced corpus beside this script")
    ap.add_argument("--rows")
    ap.add_argument("--builds", help="builds.tsv of the history index (default: REFORGE_INDEX's)")
    ap.add_argument("--seed", type=int, default=20261009)
    ap.add_argument("--check", action="append", default=[], metavar="SHA256", help="a mod whose bucket is settled")
    corpus.add_jobs(ap)
    args = ap.parse_args()
    index = corpus.env("REFORGE_INDEX")
    builds_path = args.builds or (os.path.join(index, "builds.tsv") if index else None)
    if not args.run and not args.rows:
        ap.error("RUN or --rows is required")
    os.makedirs(args.out, exist_ok=True)

    if args.rows:
        with open(args.rows, encoding="utf-8") as f:
            rows = [json.loads(line) for line in f if line.strip()]
        with open(os.path.join(os.path.dirname(args.rows), "reports.jsonl"), encoding="utf-8") as f:
            mods = dict(json.loads(line) for line in f if line.strip())
    else:
        builds = build_names(builds_path)
        shas = corpus.chosen(args, set(corpus.reports(args.run)))
        rows, mods, owns = [], {}, {}
        for sha, facts, found, own in corpus.over(of_file, [(args.run, sha, builds) for sha in shas], args.jobs):
            mods[sha] = facts
            rows += found
            if found:
                owns[sha] = own
        family = families(owns)
        for row in rows:
            row["family"] = family[row["sha256"]][:12]
        corpus.write(os.path.join(args.out, "rows.jsonl"), "".join(json.dumps(row, ensure_ascii=True) + "\n" for row in rows))
        corpus.write(os.path.join(args.out, "reports.jsonl"), "".join(json.dumps([sha, mods[sha]], ensure_ascii=True) + "\n" for sha in shas))
    write_tables(args.out, rows, mods, args.seed, sorted(args.check))


if __name__ == "__main__":
    main()
