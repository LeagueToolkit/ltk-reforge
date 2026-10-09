"""The names a port's units reach that nothing loaded at their target holds and the game's bins
of another skin hold (#GAME-BEHAVIOR-0006): copied by a run under #ADR-0083, as its
`object.copied` findings name them; before it, named in its `object.link-dangling` findings
with what copies would bring.

held_elsewhere.py RUN --out DIR [--types FILE] [--jobs N]

Inputs:

  RUN      a run directory of `run.py port`
  --types  hashes.bintypes.txt of the CommunityDragon tables, to name the classes (default:
           REFORGE_TYPES, else none: classes are named by their hashes)

Outputs, in DIR:

  units.tsv    one unit with names nothing loaded at its target holds per line: SHA-256,
               unit, its target, its source slot, its origin's slot, those names, the ones held
               elsewhere and at which slots (the slot copied from, under #ADR-0083), what copies
               bring (files and names nothing holds only before #ADR-0083)
  summary.txt  the counts"""
import argparse, collections, json, os, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import corpus  # noqa: E402

SIZES = [(0, "0"), (5, "1-5"), (20, "6-20"), (100, "21-100")]


def size_of(n):
    return next((name for top, name in SIZES if n <= top), ">100")


def of_mod(item):
    """One row per unit with names nothing loaded at its target holds."""
    run, sha = item
    report = corpus.read(run, sha)
    units = {u["id"]: u for u in report.get("units") or []}
    decisions = {d["unit"]: d for d in (report.get("targets") or {}).get("decisions") or []}
    found = {}
    for d in report.get("diagnostics") or []:
        if d["code"] not in ("object.link-dangling", "object.copied"):
            continue
        data = d["data"]
        unit = data["unit"]
        origin = (decisions.get(unit) or {}).get("origin") or {}
        row = found.setdefault(unit, {
            "sha256": sha,
            "unit": unit,
            "character": units[unit]["character_name"],
            "target": data["slot"],
            "source": units[unit]["slot"],
            "origin_slot": origin.get("slot") if origin.get("verdict") == "premium" else None,
            "names": 0,
            "elsewhere": [],
            "objects": 0,
            "files": 0,
            "unresolved": 0,
        })
        if d["code"] == "object.copied":
            row["names"] += len(data["objects"])
            row["elsewhere"] += [[copied["name"], copied["class"], [copied["from"]]] for copied in data["objects"]]
            row["objects"] += len(data["objects"])
            continue
        row["names"] += len(data["names"])
        row["elsewhere"] += [[held["name"], held["class"], held["slots"]] for held in data.get("elsewhere") or []]
        brought = data.get("brought") or {}
        for key in ("objects", "files", "unresolved"):
            row[key] += brought.get(key, 0)
    return sha, list(found.values())


def class_names(path):
    """Class names by the hash the tables spell them with, eight hex digits."""
    names = {}
    if path and os.path.exists(path):
        with open(path, encoding="utf-8", errors="replace") as f:
            for line in f:
                cells = line.rstrip("\n").split(" ", 1)
                if len(cells) == 2:
                    names[cells[0].lower().rjust(8, "0")] = cells[1]
    return names


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("run")
    ap.add_argument("--out", required=True)
    ap.add_argument("--types", default=os.environ.get("REFORGE_TYPES"))
    ap.add_argument("--jobs", type=int, default=12)
    args = ap.parse_args()
    os.makedirs(args.out, exist_ok=True)
    names = class_names(args.types)

    shas = corpus.reports(args.run)
    rows = []
    for _, found in corpus.over(of_mod, [(args.run, sha) for sha in shas], args.jobs):
        rows += found
    rows.sort(key=lambda row: (row["sha256"], row["unit"]))

    lines = ["sha256\tunit\tcharacter\ttarget\tsource\torigin_slot\tnames\telsewhere\tobjects\tfiles\tunresolved"]
    for row in rows:
        held = json.dumps(row["elsewhere"], separators=(",", ":"))
        cells = [row["sha256"], row["unit"], row["character"], row["target"], row["source"], row["origin_slot"], row["names"], held, row["objects"], row["files"], row["unresolved"]]
        lines.append("\t".join("" if cell is None else str(cell) for cell in cells))
    corpus.write(os.path.join(args.out, "units.tsv"), "\n".join(lines) + "\n")

    held_rows = [row for row in rows if row["elsewhere"]]
    held_names = sum(len(row["elsewhere"]) for row in rows)
    at_origin = [row for row in held_rows if row["origin_slot"] is not None and any(row["origin_slot"] in slots for _, _, slots in row["elsewhere"])]
    paste = [row for row in held_rows if row["origin_slot"] is not None and row["source"] == row["target"] != row["origin_slot"]]
    classes = collections.Counter(names.get(cls, cls) for row in rows for _, cls, _ in row["elsewhere"])
    sizes = collections.Counter(size_of(row["objects"]) for row in held_rows)
    out = [
        f"reports: {len(shas)}; units with dangling names: {len(rows)} in {len({r['sha256'] for r in rows})} mods, {sum(r['names'] for r in rows)} names",
        f"units with names other skins hold: {len(held_rows)} in {len({r['sha256'] for r in held_rows})} mods, {held_names} names",
        f"  of them, held at the unit's premium origin: {len(at_origin)} units in {len({r['sha256'] for r in at_origin})} mods",
        f"  of them, pastes (source slot at the target, origin elsewhere): {len(paste)} units in {len({r['sha256'] for r in paste})} mods",
        f"what copies would bring: {sum(r['objects'] for r in held_rows)} objects, {sum(r['files'] for r in held_rows)} files, {sum(r['unresolved'] for r in held_rows)} names nothing holds",
        "units by the objects copies would bring: " + ", ".join(f"{name} {sizes[name]}" for _, name in SIZES + [(None, '>100')] if sizes[name]),
        "",
        "classes of the names held elsewhere: names",
    ]
    out += [f"  {name:44} {n:6}" for name, n in classes.most_common(15)]
    text = "\n".join(out) + "\n"
    corpus.write(os.path.join(args.out, "summary.txt"), text)
    print(text, end="")


if __name__ == "__main__":
    main()
