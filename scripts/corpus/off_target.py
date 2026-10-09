"""The units a run's ports leave at slots the port does not change (#ADR-0082), and the own work
the `unit.left-with-work` findings name, against a comparison of that run with its baseline.

off_target.py RUN [--compare DIR]

Inputs:

  RUN        a run directory of `run.py port`
  --compare  the output directory of `compare.py BASE RUN`: each mod it lists as differing is
             checked to hold a unit that leaves as `off-target`

Output: printed, the counts and the mods that differ for another reason."""
import argparse, collections, os, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import corpus  # noqa: E402


def of_mod(item):
    """The units a mod's port leaves at other slots, and the rows of its work findings."""
    run, sha = item
    report = corpus.read(run, sha)
    left = ((report.get("port") or {}).get("shipping") or {}).get("left") or {}
    off = [unit for unit, why in left.items() if why.get("why") == "off-target"]
    rows = [row for d in report.get("diagnostics") or [] if d["code"] == "unit.left-with-work" for row in d["data"]["units"]]
    return sha, off, rows


def differing(directory):
    """The mods `compare.py` lists with a difference not expected."""
    with open(os.path.join(directory, "summary.txt"), encoding="utf-8") as f:
        return {line.split()[-1] for line in f if line.startswith("differs: ")}


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("run")
    ap.add_argument("--compare")
    corpus.add_jobs(ap)
    args = ap.parse_args()

    shas = corpus.reports(args.run)
    mods, units, kinds, kinds_mods, held = set(), 0, collections.Counter(), collections.defaultdict(set), collections.Counter()
    for sha, off, rows in corpus.over(of_mod, [(args.run, sha) for sha in shas], args.jobs):
        if off:
            mods.add(sha)
            units += len(off)
        for row in rows:
            kinds[row["kind"]] += 1
            kinds_mods[row["kind"]].add(sha)
            for what in ("models", "skeletons", "textures", "files", "objects", "clips"):
                held[what] += row[what]
    print(f"reports: {len(shas)}; mods with units left at other slots: {len(mods)}, units: {units}")
    for kind in sorted(kinds):
        print(f"  work-left {kind}: {kinds[kind]} units in {len(kinds_mods[kind])} mods")
    print("  own work named: " + ", ".join(f"{n} {what}" for what, n in held.items()))
    if args.compare:
        differs = differing(args.compare)
        other = sorted(differs - mods)
        print(f"differing mods: {len(differs)}, with no unit left at another slot: {len(other)}")
        for sha in other:
            print(f"  {sha}")
        sys.exit(1 if other else 0)


if __name__ == "__main__":
    main()
