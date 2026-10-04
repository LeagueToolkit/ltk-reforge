"""Picks the reduced corpus: the fewest mods that together show every case (#ADR-0045).

reduce.py --cases FILE [--out FILE] [--without PREFIX ...] [--limit N] --datasets DIR [DIR ...] --meta DIR

A greedy set cover: the mod showing the most cases not yet shown is taken, until every case
is shown. Among mods showing as many, the one with the smaller archive is taken, then the
one with the lower SHA-256, so the same cases give the same corpus.

Inputs:

  --cases     mods.json as `cases.py` writes it: the cases each mod shows
  --without   a case whose text starts with this is left out of the cover (repeatable): the
              cut for a closed set with many values when the corpus grows past the limit
  --limit     the size the corpus should stay under (default 150). A larger cover is written
              all the same, and the exit code says so
  --datasets, --meta  where the archives are (`corpus.py`), for the size of each

Output: `--out` (default: reduced.json beside this script), for `run.py --reduced` and
`compare.py --reduced`. It names each mod by its SHA-256, with the size of its archive, the
cases it was the first to show when it was taken (`first`) and, of those, the cases no other
mod of the runs shows (`only`).

Exits non-zero when the corpus is larger than the limit."""
import argparse, json, os, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import corpus  # noqa: E402


def cover(shown, sizes):
    """The mods taken, in the order taken, each with the cases it was the first to show."""
    left = set().union(*shown.values()) if shown else set()
    taken = []
    while left:
        best = min(shown, key=lambda sha: (-len(shown[sha] & left), sizes[sha], sha))
        first = shown[best] & left
        taken.append((best, first))
        left -= first
    return taken


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--cases", required=True)
    ap.add_argument("--out", default=corpus.REDUCED)
    ap.add_argument("--without", action="append", default=[])
    ap.add_argument("--limit", type=int, default=150)
    corpus.add_datasets(ap)
    args = ap.parse_args()
    corpus.require(ap, args, "datasets", "meta")

    with open(args.cases, encoding="utf-8") as f:
        listing = json.load(f)
    names = [case for _area, case in listing["cases"]]
    kept = {at for at, case in enumerate(names) if not any(case.startswith(prefix) for prefix in args.without)}
    where, _, _ = corpus.mods_in_scope(args.datasets, args.meta)
    shown = {sha: set(cases) & kept for sha, cases in listing["mods"].items() if sha in where}
    sizes = {sha: os.path.getsize(where[sha]) for sha in shown}
    holders = {}
    for sha, cases in shown.items():
        for case in cases:
            holders[case] = holders.get(case, 0) + 1

    taken = cover(shown, sizes)
    mods = []
    for sha, first in taken:
        only = sorted(names[case] for case in first if holders[case] == 1)
        mods.append({"sha256": sha, "bytes": sizes[sha], "first": sorted(names[case] for case in first), "only": only})
    reduced = {
        "runs": listing["runs"],
        "mods_in_runs": len(shown),
        "cases": len(holders),
        "without": args.without,
        "mods": mods,
    }
    corpus.write(args.out, json.dumps(reduced, indent=1) + "\n")
    rare = sum(1 for count in holders.values() if count == 1)
    print(f"{len(shown)} mods, {len(holders)} cases, {rare} of them shown by one mod; {len(names) - len(kept)} cases left out")
    print(f"reduced: {len(mods)} mods, {sum(mod['bytes'] for mod in mods)} bytes of archives; {sum(1 for mod in mods if mod['only'])} mods are the only one to show a case")
    print(f"wrote {args.out}")
    if len(mods) > args.limit:
        print(f"the corpus is over the limit of {args.limit} mods")
        sys.exit(1)


if __name__ == "__main__":
    main()
