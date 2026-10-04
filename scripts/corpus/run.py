"""Runs one reforge command over mods of the corpus, one report per mod.

run.py COMMAND --out DIR [--list FILE | --reduced] [--jobs N] [inputs] [-- ARGUMENT ...]

COMMAND is one of:

  port     reforge port --no-write: the port is built and checked, nothing is written
  repair   reforge repair --no-write --again: the repair is built, checked and repaired again
  inspect  reforge inspect: the facts and the decisions
  load     reforge load: the mod as loaded

The mods are those in scope (#ADR-0033), or the ones `--list FILE` names (one SHA-256 per
line, or a reduced corpus as `reduce.py` writes it), or the reduced corpus (`--reduced`).
What follows `--` is passed to the command.

Inputs: `corpus.py` lists them. `--datasets`, `--meta`, `--game` and `--index` are required.

Outputs, in DIR:

  <sha256>.json  the report of each mod. One already there is kept, so a run resumes;
                 delete a report to run its mod again
  <sha256>.err   the exit code and the output of a run that left no report
  run.log        the command, then one line per mod run: SHA-256, exit code, seconds
  summary.txt    how many mods were listed, how many have a report, how many runs ended
                 done (exit 0), how many a fatal diagnostic ended (exit 2) and by which
                 code, and the mods with no report, each with its exit code

Exits non-zero when a mod has no report."""
import argparse, collections, os, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import corpus  # noqa: E402

COMMANDS = {
    "port": ["--no-write"],
    "repair": ["--no-write", "--again"],
    "inspect": [],
    "load": [],
}


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("command", choices=sorted(COMMANDS))
    ap.add_argument("--out", required=True)
    ap.add_argument("--list", help="a text file of SHA-256s, or a reduced corpus")
    ap.add_argument("--reduced", action="store_true", help="the reduced corpus beside this script")
    corpus.add_datasets(ap)
    corpus.add_program(ap)
    ap.add_argument("extra", nargs="*", help="after `--`: arguments passed to the command")
    args = ap.parse_args()
    corpus.require(ap, args, "datasets", "meta", "game", "index")

    where, classed, in_scope = corpus.mods_in_scope(args.datasets, args.meta)
    print(f"datasets: {classed} mods classed, {in_scope} in scope, {len(where)} found", flush=True)
    shas = corpus.chosen(args, where)

    inputs = ["--game", args.game, "--index", args.index]
    inputs += ["--mimir", args.mimir] if args.mimir else []
    inputs += ["--schema", args.schema] if args.schema else []
    options = inputs + COMMANDS[args.command] + args.extra

    def command(sha, report):
        return [args.reforge, args.command, where[sha]] + options + ["--report", report]

    os.makedirs(args.out, exist_ok=True)
    with open(os.path.join(args.out, "run.log"), "a", encoding="utf-8", newline="\n") as log:
        log.write(f"# {args.command} <archive> {' '.join(options)}\n")
        exits, seconds = corpus.run_mods(shas, args.out, command, args.jobs, log)
        log.write(f"# {len(exits)} mods run in {seconds:.0f}s at {args.jobs} in parallel\n")

    have = set(corpus.reports(args.out))
    missing = [sha for sha in shas if sha not in have]
    halts = collections.Counter(corpus.halted(args.out, sha) for sha in shas if sha in have)
    done = halts.pop(None, 0)
    lines = [
        f"command: {args.command} {' '.join(COMMANDS[args.command] + args.extra)}".rstrip(),
        f"mods: {len(shas)} listed, {len(shas) - len(missing)} with a report, {len(missing)} with none",
        f"exit {corpus.DONE}, done: {done}",
        f"exit {corpus.HALTED}, ended by a fatal diagnostic: {sum(halts.values())}",
    ]
    lines += [f"  {code:32} {n:6}" for code, n in sorted(halts.items())]
    lines += [f"no report: {sha} ({corpus.failure(args.out, sha)})" for sha in missing]
    text = "\n".join(lines) + "\n"
    corpus.write(os.path.join(args.out, "summary.txt"), text)
    print(text, end="")
    print(f"{len(exits)} mods run in {seconds:.0f}s", flush=True)
    sys.exit(1 if missing else 0)


if __name__ == "__main__":
    main()
