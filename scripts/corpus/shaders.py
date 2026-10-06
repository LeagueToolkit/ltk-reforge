"""The shaders mods link and define, against the game's shader bin (#GAME-CHANGE-0024,
#GAME-BEHAVIOR-0022, #GAME-BEHAVIOR-0023).

shaders.py --out DIR [--list FILE | --reduced] [--run RUN] [--tables DIR] [--jobs N] [inputs]

Runs the example `shader_links` over the mods in scope (#ADR-0033), or the ones `--list` or
`--reduced` names, each archive read in memory. Build it first:
`cargo build --release -p reforge --example shader_links`.

  --run     a run directory of `run.py`. Only the bins its reports say the client loads count
  --tables  a directory of CommunityDragon text tables: `hashes.binentries.txt` spells the shaders

Inputs: `corpus.py` lists them. `--datasets`, `--meta` and `--game` are required.

Outputs, in DIR:

  links.tsv    the example's rows: SHA-256, WAD, bin, `def` or `link`, entry, shader, whether
               the game's shader bin holds that shader
  summary.txt  the mods whose material links a shader the game's bin lacks, by shader, and the
               mods that define a `CustomShaderDef`, by whether the game's bin holds its name"""
import argparse, collections, concurrent.futures, os, subprocess, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import corpus  # noqa: E402

EXAMPLE = os.path.join(corpus.ROOT, "target", "release", "examples", "shader_links" + (".exe" if os.name == "nt" else ""))
# Archives per run of the example: few enough that the parts spread over the jobs.
PART = 20


def loaded_bins(run, shas):
    """Per mod, the bins its report says the client loads, as (WAD name lowercased, path hash)."""
    found = {}
    for sha in shas:
        if not os.path.exists(os.path.join(run, sha + ".json")):
            continue
        report = corpus.read(run, sha)
        wads = {w["id"]: w["name"].lower() for w in report.get("wads") or []}
        found[sha] = {
            (wads.get(f["wad"], ""), f["hash"])
            for f in report.get("files") or []
            if f.get("use") == "loaded"
        }
    return found


def names(tables):
    """Entry names by hash, from `hashes.binentries.txt`."""
    spelled = {}
    if not tables:
        return spelled
    with open(os.path.join(tables, "hashes.binentries.txt"), encoding="utf-8") as f:
        for line in f:
            h, _, name = line.rstrip("\n").partition(" ")
            spelled[h] = name
    return spelled


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--out", required=True)
    ap.add_argument("--list", help="a text file of SHA-256s, or a reduced corpus")
    ap.add_argument("--reduced", action="store_true", help="the reduced corpus beside this script")
    ap.add_argument("--run", help="a run directory whose reports say which bins are loaded")
    ap.add_argument("--tables", help="a directory of CommunityDragon text tables")
    corpus.add_datasets(ap)
    corpus.add_program(ap)
    args = ap.parse_args()
    corpus.require(ap, args, "datasets", "meta", "game")

    where, classed, in_scope = corpus.mods_in_scope(args.datasets, args.meta)
    print(f"datasets: {classed} mods classed, {in_scope} in scope, {len(where)} found", flush=True)
    shas = corpus.chosen(args, where)
    parts_dir = os.path.join(args.out, "parts")
    os.makedirs(parts_dir, exist_ok=True)

    def one(index):
        listing = os.path.join(parts_dir, f"{index:05}.txt")
        rows = os.path.join(parts_dir, f"{index:05}.tsv")
        with open(listing, "w", encoding="utf-8", newline="\n") as f:
            f.write("\n".join(where[sha] for sha in shas[index : index + PART]) + "\n")
        with open(rows, "w", encoding="utf-8", newline="\n") as out:
            done = subprocess.run([EXAMPLE, args.game, listing], stdout=out, stderr=subprocess.PIPE, encoding="utf-8", errors="replace")
        if done.returncode != 0:
            print(f"part {index}: exit {done.returncode}: {done.stderr.strip()[-300:]}", flush=True)
        return rows

    with concurrent.futures.ThreadPoolExecutor(args.jobs) as pool:
        parts = list(pool.map(one, range(0, len(shas), PART)))
    rows = []
    for part in parts:
        with open(part, encoding="utf-8") as f:
            rows += [line.rstrip("\n").split("\t") for line in f if not line.startswith("sha256\t")]
    with open(os.path.join(args.out, "links.tsv"), "w", encoding="utf-8", newline="\n") as f:
        f.write("sha256\twad\tchunk\tkind\tentry\tshader\tin_game\n")
        f.writelines("\t".join(row) + "\n" for row in rows)

    loaded = loaded_bins(args.run, shas) if args.run else None
    if loaded is not None:
        rows = [r for r in rows if (r[1].lower(), r[2]) in loaded.get(r[0], set())]
    spell = names(args.tables)
    missing = collections.defaultdict(set)
    missing_links = collections.Counter()
    defined = collections.defaultdict(set)
    for sha, _wad, _chunk, kind, _entry, shader, in_game in rows:
        if kind == "link" and in_game == "false":
            missing[shader].add(sha)
            missing_links[shader] += 1
        elif kind == "def":
            defined[("a game shader's name" if in_game == "true" else "a name of its own", shader)].add(sha)
    linking = set().union(*missing.values()) if missing else set()
    defining = set().union(*defined.values()) if defined else set()
    lines = [
        f"mods: {len(shas)} listed" + (f", the bins {args.run} says are loaded" if args.run else ", every bin"),
        f"mods whose material links a shader the game's bin lacks: {len(linking)}",
        f"mods that define a CustomShaderDef: {len(defining)}",
        f"mods of both: {len(linking & defining)}",
        "",
        "shaders linked that the game's bin lacks: mods, links, shader",
    ]
    for shader in sorted(missing, key=lambda s: (-len(missing[s]), spell.get(s, s))):
        lines.append(f"  {len(missing[shader]):5} {missing_links[shader]:6}  {spell.get(shader, '0x' + shader)}")
    lines += ["", "CustomShaderDef objects of mods: mods, under, shader"]
    for (under, shader), mods in sorted(defined.items(), key=lambda kv: (-len(kv[1]), kv[0])):
        lines.append(f"  {len(mods):5}  {under}: {spell.get(shader, '0x' + shader)}")
    with open(os.path.join(args.out, "summary.txt"), "w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(lines) + "\n")
    print("\n".join(lines[:4]))


if __name__ == "__main__":
    main()
