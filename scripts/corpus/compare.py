"""Compares two runs of reforge over the same mods, a baseline and a new one (#ADR-0044).

compare.py BASE NEW --out DIR [--list FILE | --reduced] [--expect FILE] [--jobs N]

Each mod's two reports are compared section by section. A WAD, a file, a character and a unit
are named by what they are (a WAD's name, a file's WAD and path hash, a character's name, a
unit's character and slot), so an id that moved is no difference. The sections:

  parts        the parts the report holds: one that came or went, whichever section reads it
  run          the diagnostic that ended the run, the install's build, the history's head,
               the schema's build and a digest of the hash tables' stamp
  wads, files, characters, dating, inventory, provenance
               the mod as loaded, row by row and field by field
  units        per unit: its bin and entry, whether it ships, why it left the port otherwise
  facts        per unit and table (copies, origins, rigs, clips, candidates, sounds): a digest
  decisions    per unit: origin, target, reason, and each candidate with its checks
  plan         per unit: what its entry reaches, its place, its audio; the HUD edits
  refs         per unit and file named: where the file is found
  renames      the units retargeted, the names made, the place each file ships at
  chunks       per layer, WAD and path: origin and bytes, and for a bin each object by entry
  patches      per layer and game bin: the PTCH, its links, the outcome of applying it, and
               each object of the patched bin by entry
  package      the project's name and version, and the names its hash table holds
  fixups       per fixup, object and path: the kinds before and after, then the values
  left-behind  the counts of the files a package leaves out, by reason
  again        what repairing the repaired mod again changed
  evidence     the evidence the package embeds (#ADR-0041), field by field
  diagnostics  per code and subject: how many, then their data

Inputs: BASE and NEW, two run directories of `run.py`. The mods compared are those NEW holds a
report of, or the ones `--list` or `--reduced` name. `--expect FILE` holds the kinds of
difference a change under work is meant to make, one per line: a section, a field (`*` for
any) and optionally a regular expression searched in the key. Lines starting with `#` are
comments. There is no list of accepted deviations: a difference is meant, and then the
baseline is replaced, or it is a fault.

Outputs, in DIR:

  differences.tsv  one line per difference, sorted: SHA-256, section, key, field, the
                   baseline's value, the new value, and `expected` where `--expect` takes it
  summary.txt      how many mods are equal, and the differences and the mods by section and
                   field, and by diagnostic code

Exits non-zero when a difference is not expected or a listed mod lacks a report."""
import argparse, collections, hashlib, json, os, re, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import corpus  # noqa: E402

INSTALL = re.compile(rb'\n    "install": "[^"\n]*",?\r?\n')
ID = re.compile(r"^%[wfcu]\d+$")
FACTS = ["copies", "origins", "rigs", "clips", "candidates", "sounds"]
LEFT_BEHIND = {"package.ported", "package.in-place"}


def named(report):
    """A report with each id of a WAD, a file, a character and a unit replaced by its name."""
    names = {}
    for wad in report.get("wads") or []:
        names[wad["id"]] = wad["name"].lower()
    for file in report.get("files") or []:
        names[file["id"]] = f"{names.get(file['wad'])}:{file['hash']}"
    for character in report.get("characters") or []:
        names[character["id"]] = character["name"]
    for unit in report.get("units") or []:
        names[unit["id"]] = f"{unit['character_name']} skin{unit['slot']}"

    def name(value):
        if isinstance(value, str):
            return names.get(value, value) if ID.match(value) else value
        if isinstance(value, list):
            return [name(item) for item in value]
        if isinstance(value, dict):
            return {names.get(key, key): name(item) for key, item in value.items()}
        return value

    return name(report)


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True).encode()).hexdigest()[:12]


def rows(base, new):
    """The differences of two tables of rows, key to fields: a row one side lacks, then each
    field that differs."""
    for key in sorted(set(base) | set(new)):
        a, b = base.get(key), new.get(key)
        if a is None or b is None:
            yield key, "present", a is not None, b is not None
            continue
        for field in sorted(set(a) | set(b)):
            if a.get(field) != b.get(field):
                yield key, field, a.get(field), b.get(field)


def without(row, *fields):
    return {key: value for key, value in row.items() if key not in fields}


def halt(report):
    by_id = {d["id"]: d["code"] for d in report.get("diagnostics") or []}
    return by_id.get(report.get("halted"))


def run(report):
    inputs = report.get("inputs") or {}
    history = inputs.get("history") or {}
    schema = inputs.get("schema") or {}
    names = inputs.get("names")
    names = hashlib.sha256(json.dumps(names, sort_keys=True).encode()).hexdigest()[:16] if names is not None else None
    return {"-": {"halted": halt(report), "build": inputs.get("build"), "history": history.get("head"), "schema": report.get("schema"), "schema_build": schema.get("latest"), "names": names}}


def wads(report):
    return {wad["id"]: without(wad, "id") for wad in report.get("wads") or []}


def files(report):
    return {file["id"]: without(file, "id", "wad", "hash") for file in report.get("files") or []}


def characters(report):
    return {c["name"]: without(c, "id", "name") for c in report.get("characters") or []}


def dating(report):
    return {"-": report["dating"]} if report.get("dating") else {}


def inventory(report):
    found = report.get("inventory") or {}
    table = {f"kind {kind}": count for kind, count in (found.get("by_kind") or {}).items()}
    items = collections.Counter(f"item {i['finding']} {i['kind']} {i['place']} {i['name']}" for i in found.get("items") or [])
    table.update({key: {"count": count} for key, count in items.items()})
    return table


def provenance(report):
    return {row["file"]: without(row, "file") for row in report.get("provenance") or []}


def units(report):
    """Each unit as loaded, and whether it ships: `ships`, or why it left the port."""
    shipping = (report.get("port") or {}).get("shipping")
    table = {}
    for unit in report.get("units") or []:
        row = {"file": unit["file"], "bin": unit["bin"] is not None, "entry": unit["entry"] is not None}
        row.update(loads=unit["loads"], entry_name_slot=unit["entry_name_slot"])
        if shipping is not None:
            left = shipping["left"].get(unit["id"])
            row["state"] = without(left, "diagnostic") if left else "ships" if unit["id"] in shipping["ships"] else None
        table[unit["id"]] = row
    return table


def facts(report):
    found = report.get("facts")
    if not found:
        return {}
    table = collections.defaultdict(dict)
    for name in FACTS:
        for row in found.get(name) or []:
            table[row["unit"]][name] = digest(row)
    objects = found.get("objects") or {}
    table["-"] = {"objects": objects.get("total"), "own": objects.get("own"), "game": digest(objects.get("game")), "reworks": digest(found.get("reworks"))}
    return dict(table)


def decisions(report):
    targets = report.get("targets")
    if not targets:
        return {}
    table = {"-": {"policy": targets["policy"], "conflicts": targets["conflicts"]}}
    for decision in targets["decisions"]:
        row = without(decision, "unit", "character", "candidates", "alternatives", "policy")
        for candidate in decision["candidates"]:
            checks = [[check["aspect"], check["fit"], check["why"]] for check in candidate["checks"]]
            row[f"candidate {candidate['slot']}"] = [candidate["feasible"], candidate["why"], checks]
        for other in decision["alternatives"]:
            row[f"alternative {other['policy']['target']} {other['policy']['anim_reuse']}"] = [other["chosen"], other["reason"]]
        table[decision["unit"]] = row
    return table


def plan(report):
    port = report.get("port")
    if not port:
        return {}
    table = collections.defaultdict(dict)
    table["-"] = {"options": port["options"], "namespace": port["layout"]["namespace"], "repointed": port["repointed"]}
    for unit in port["units"]:
        table[unit["unit"]].update(without(unit, "unit", "carried"), carried=len(unit["carried"]))
    for unit, reach in port["reach"]["units"].items():
        fates = collections.Counter(fate for _, fate in reach["objects"])
        row = without(reach, "objects", "leftovers", "game", "champion")
        row.update(reached=dict(fates), leftovers=len(reach["leftovers"]), game=len(reach["game"]), champion=len(reach["champion"]))
        table[unit].update(row)
    for audio in port["audio"]["units"]:
        table[audio["unit"]]["audio"] = without(audio, "unit")
    for hud in port["hud"]["champions"]:
        table[f"hud {hud['character']}"] = without(hud, "character")
    return dict(table)


def refs(report):
    port = report.get("port")
    if not port:
        return {}
    return {f"{unit} {path}": found for unit, paths in port["refs"]["units"].items() for path, found in paths.items()}


def renames(report):
    port = report.get("port")
    if not port:
        return {}
    made = port["renames"]
    table = {"-": {"links_kept": made["links_kept"], "to_target": made["to_target"], "game_swaps": made["game_swaps"]}}
    for unit in made["retargeted"]:
        table[f"unit {unit['unit']}"] = without(unit, "unit")
    for name, count in collections.Counter(made["names"].values()).items():
        table[f"name {name}"] = {"count": count}
    for file, place in port["files"].items():
        table[f"file {file}"] = {"place": place}
    return table


def chunk_key(chunk):
    return f"{chunk['layer']} {chunk['wad'].lower()} {chunk['path']}"


def chunks(base, new):
    """Each chunk by layer, WAD and path. Two bins that both read are compared object by
    object, and by their bytes when every object is equal."""
    ours, theirs = [{chunk_key(c): c for c in (r.get("package") or {}).get("chunks") or []} for r in (base, new)]
    for key in sorted(set(ours) | set(theirs)):
        a, b = ours.get(key), theirs.get(key)
        if a is None or b is None:
            yield key, "present", a is not None, b is not None
            continue
        for field in ("origin", "file", "edited"):
            if a.get(field) != b.get(field):
                yield key, field, a.get(field), b.get(field)
        objects = (a.get("objects"), b.get("objects"))
        differing = 0
        if None not in objects:
            for entry in sorted(set(objects[0]) | set(objects[1])):
                if objects[0].get(entry) != objects[1].get(entry):
                    differing += 1
                    yield f"{key} {entry}", "object", objects[0].get(entry), objects[1].get(entry)
            if list(objects[0]) != list(objects[1]) and not differing:
                differing += 1
                yield key, "object order", digest(list(objects[0])), digest(list(objects[1]))
        if a["content"] != b["content"] and not differing:
            yield key, "bytes", a["content"], b["content"]


def patches(base, new):
    """Each patch by layer and game bin: the PTCH, the links, and the patched bin by object."""
    ours, theirs = [{f"{p['layer']} {p['target']}": p for p in (r.get("package") or {}).get("patches") or []} for r in (base, new)]
    for key in sorted(set(ours) | set(theirs)):
        a, b = ours.get(key), theirs.get(key)
        if a is None or b is None:
            yield key, "present", a is not None, b is not None
            continue
        for field in sorted(set(a) | set(b)):
            if field not in ("applied", "content", "size") and a.get(field) != b.get(field):
                yield key, field, a.get(field), b.get(field)
        applied = (a.get("applied") or {}, b.get("applied") or {})
        differing = 0
        for field in ("outcome", "reason", "skipped", "links"):
            if applied[0].get(field) != applied[1].get(field):
                differing += 1
                yield key, f"applied {field}", applied[0].get(field), applied[1].get(field)
        objects = (applied[0].get("objects") or {}, applied[1].get("objects") or {})
        for entry in sorted(set(objects[0]) | set(objects[1])):
            if objects[0].get(entry) != objects[1].get(entry):
                differing += 1
                yield f"{key} {entry}", "object", objects[0].get(entry), objects[1].get(entry)
        if a["content"] != b["content"] and not differing:
            yield key, "bytes", a["content"], b["content"]


def package(report):
    built = report.get("package")
    if not built:
        return {}
    return {"-": {"name": built["name"], "version": built["version"], "paths": digest(built["paths"]), "entries": digest(built["entries"])}}


def fixups(base, new):
    """Each change by fixup, object and path: the kinds before and after, then the values."""

    def changes(report):
        table = collections.defaultdict(list)
        for row in report.get("changes") or []:
            for change in row["changes"]:
                before, after = change["before"] or {}, change["after"]
                kinds = [before.get("kind"), after["as"], after.get("kind") or after.get("wants"), change["reached"]]
                values = [before.get("text"), after.get("text") or after.get("to")]
                table[f"{row['fixup']} {change['entry']} {change['path']}"].append((kinds, values))
        return table

    ours, theirs = changes(base), changes(new)
    for key in sorted(set(ours) | set(theirs)):
        a, b = sorted(ours.get(key, [])), sorted(theirs.get(key, []))
        if [kinds for kinds, _ in a] != [kinds for kinds, _ in b]:
            yield key, "kinds", [kinds for kinds, _ in a], [kinds for kinds, _ in b]
        elif a != b:
            yield key, "values", [values for _, values in a], [values for _, values in b]


def left_behind(report):
    return {d["code"]: d["data"] for d in report.get("diagnostics") or [] if d["code"] in LEFT_BEHIND}


def again(report):
    found = report.get("again")
    if not found:
        return {}
    values = {row["fixup"]: row["values"] for row in found["fixups"]}
    return {"-": {"built": found["built"], "redated": found.get("redated"), "fixups": values, "chunks_differing": found["chunks_differing"]}}


def parts(report):
    """The parts a report holds, so that one no section reads still shows when it comes or goes."""
    return {"-": {part: True for part in report}}


def evidence(report):
    return {"-": report["evidence"]} if report.get("evidence") else {}


def diagnostics(base, new):
    """How many diagnostics each code raises about each subject, then what they hold."""

    def raised(report):
        table = collections.defaultdict(list)
        for d in report.get("diagnostics") or []:
            data = None if d["code"] in LEFT_BEHIND else d["data"]
            table[f"{d['code']} {d['subject']['kind']} {d['subject']['name']}"].append(json.dumps([d.get("by"), data], sort_keys=True))
        return table

    ours, theirs = raised(base), raised(new)
    for key in sorted(set(ours) | set(theirs)):
        a, b = sorted(ours.get(key, [])), sorted(theirs.get(key, []))
        if len(a) != len(b):
            yield key, "count", len(a), len(b)
        elif a != b:
            yield key, "data", digest(a), digest(b)


# A section's name, and how it is compared: a function that gives one report's rows, or one
# that takes both reports and gives the differences.
BY_ROWS = [
    ("parts", parts), ("run", run), ("wads", wads), ("files", files), ("characters", characters),
    ("dating", dating), ("inventory", inventory), ("provenance", provenance), ("units", units),
    ("facts", facts), ("decisions", decisions), ("plan", plan), ("refs", refs), ("renames", renames),
    ("package", package), ("left-behind", left_behind), ("again", again), ("evidence", evidence),
]  # fmt: skip
BY_PAIR = [("chunks", chunks), ("patches", patches), ("fixups", fixups), ("diagnostics", diagnostics)]
SECTIONS = [name for name, _ in BY_ROWS[:14]] + ["chunks", "patches", "package", "fixups", "left-behind", "again", "evidence", "diagnostics"]


def differences(base, new):
    """Every difference of two reports of one mod: section, key, field, the two values."""
    base, new = named(base), named(new)
    found = []
    for section, table in BY_ROWS:
        found += [(section, *difference) for difference in rows(table(base), table(new))]
    for section, differ in BY_PAIR:
        found += [(section, *difference) for difference in differ(base, new)]
    return found


def of_mod(item):
    """One mod's differences. Reports equal byte for byte, but for the place of the install,
    are not parsed."""
    base, new, sha = item
    with open(os.path.join(base, sha + ".json"), "rb") as f:
        ours = f.read()
    with open(os.path.join(new, sha + ".json"), "rb") as f:
        theirs = f.read()
    if INSTALL.sub(b"\n", ours) == INSTALL.sub(b"\n", theirs):
        return sha, []
    return sha, differences(json.loads(ours), json.loads(theirs))


def expectations(path):
    """The kinds of difference a file says are meant: section, field, and a pattern of keys."""
    meant = []
    if path:
        with open(path, encoding="utf-8") as f:
            for line in f:
                parts = line.split(None, 2)
                if parts and not parts[0].startswith("#"):
                    parts += ["*"] * (2 - len(parts)) if len(parts) < 2 else []
                    meant.append((parts[0], parts[1], re.compile(parts[2].strip()) if len(parts) > 2 else None))
    return meant


def expected(meant, section, key, field):
    for wanted, wanted_field, pattern in meant:
        if wanted in ("*", section) and wanted_field in ("*", field) and (pattern is None or pattern.search(key)):
            return True
    return False


def cell(value):
    text = json.dumps(value, sort_keys=True, ensure_ascii=True)
    return text if len(text) <= 300 else text[:297] + "..."


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("base")
    ap.add_argument("new")
    ap.add_argument("--out", required=True)
    ap.add_argument("--list", help="a text file of SHA-256s, or a reduced corpus")
    ap.add_argument("--reduced", action="store_true", help="the reduced corpus beside this script")
    ap.add_argument("--expect", help="the kinds of difference that are meant")
    corpus.add_jobs(ap)
    args = ap.parse_args()
    os.makedirs(args.out, exist_ok=True)

    in_base, in_new = set(corpus.reports(args.base)), set(corpus.reports(args.new))
    path = corpus.REDUCED if args.reduced else args.list
    shas = corpus.listed(path) if path else sorted(in_new)
    no_base = [sha for sha in shas if sha not in in_base]
    no_new = [sha for sha in shas if sha not in in_new]
    both = [sha for sha in shas if sha in in_base and sha in in_new]
    meant = expectations(args.expect)

    by_field = collections.defaultdict(lambda: [0, set()])
    by_code = collections.defaultdict(lambda: [0, set()])
    as_expected = collections.defaultdict(lambda: [0, set()])
    differing, faulty, lines = set(), set(), []
    for sha, found in corpus.over(of_mod, [(args.base, args.new, sha) for sha in both], args.jobs):
        for section, key, field, a, b in found:
            differing.add(sha)
            meant_so = expected(meant, section, key, field)
            lines.append((sha, SECTIONS.index(section), section, key, field, cell(a), cell(b), "expected" if meant_so else ""))
            tally = as_expected if meant_so else by_field
            tally[(section, field)][0] += 1
            tally[(section, field)][1].add(sha)
            if not meant_so:
                faulty.add(sha)
                if section == "diagnostics":
                    code = key.split(" ", 1)[0]
                    by_code[code][0] += 1
                    by_code[code][1].add(sha)
    lines.sort()
    text = "sha256\tsection\tkey\tfield\tbase\tnew\texpected\n"
    text += "".join("\t".join((sha, section, key, field, a, b, meant_so)) + "\n" for sha, _, section, key, field, a, b, meant_so in lines)
    corpus.write(os.path.join(args.out, "differences.tsv"), text)

    out = [
        f"mods: {len(shas)} listed, {len(both)} compared, {len(both) - len(differing)} equal, {len(differing)} with a difference, {len(faulty)} with one not expected",
        f"not compared: {len(no_base)} with no baseline report, {len(no_new)} with no new report",
        f"differences: {len(lines)}, {sum(n for n, _ in as_expected.values())} of them expected",
        "",
    ]
    for title, tally in (("differences not expected, by section and field", by_field), ("differences expected, by section and field", as_expected)):
        if tally or tally is by_field:
            out.append(f"{title}: differences, mods")
            keys = sorted(tally, key=lambda key: (SECTIONS.index(key[0]), key[1]))
            out += [f"  {section + ' ' + field:44} {tally[(section, field)][0]:8} {len(tally[(section, field)][1]):6}" for section, field in keys]
            out.append("")
    out.append("differences not expected in diagnostics, by code: differences, mods")
    out += [f"  {code:44} {n:8} {len(mods):6}" for code, (n, mods) in sorted(by_code.items())]
    out.append("")
    out += [f"differs: {sha}" for sha in sorted(faulty)]
    out += [f"differs as expected: {sha}" for sha in sorted(differing - faulty)]
    out += [f"no baseline report: {sha}" for sha in no_base]
    out += [f"no new report: {sha}" for sha in no_new]
    text = "\n".join(out).rstrip("\n") + "\n"
    corpus.write(os.path.join(args.out, "summary.txt"), text)
    print(text, end="")
    sys.exit(1 if faulty or no_base or no_new else 0)


if __name__ == "__main__":
    main()
