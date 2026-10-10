"""What the corpus scripts share: the mods in scope (#ADR-0033), where each archive is, the
runner, and the reports of a run.

Inputs, each an argument, else an environment variable:

  --datasets  REFORGE_DATASETS      directories of archives, each named <sha256>.fantome
                                    (several directories, separated as PATH is)
  --meta      REFORGE_DATASET_META  the directory of mod_classes.tsv and dedup_map.tsv
  --reforge   REFORGE_EXE           the program (default: target/release/reforge of this checkout)
  --game      REFORGE_GAME          the game install the mods are judged against
  --index     REFORGE_INDEX         the history index
  --mimir     REFORGE_MIMIR         the hash cache's directory (default: the program's own)
  --schema    REFORGE_SCHEMA        the meta schema database (default: the program's own)
  --store     REFORGE_STORE         a directory the program keeps each archive's raw snapshot
                                    and files in, and reads them from in the runs after
                                    (#ADR-0095); a directory under data/corpus/. None by default:
                                    each run reads every archive in memory

mod_classes.tsv holds one mod per line: SHA-256, dataset, class, how many characters, the
characters separated by commas. dedup_map.tsv holds one byte-identical archive per line: its
SHA-256, then the SHA-256 of the one kept.

A run is a directory of reports, <sha256>.json, one per mod. A mod is named by its SHA-256
everywhere. No archive is extracted: the program reads it in memory and writes a report.

Outputs: none of its own."""
import concurrent.futures, json, os, re, subprocess, time

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
HERE = os.path.dirname(os.path.abspath(__file__))
REDUCED = os.path.join(HERE, "reduced.json")

# #ADR-0033: the classes a port is defined for.
IN_SCOPE = {"champion-skin", "audio+skin"}
# The class table names a mod's WAD folders as its characters. These are no champion's: maps,
# items, UI. An odd name that may still be a champion's stays, the program finds champions
# by skin bin.
NOT_CHAMPION = re.compile(r"^(map\d+\w*|global|data\d*|item|summoner_spells|companions|ui|shadercache|bootstrap)$|rift|minimap")
# #ADR-0033: an all-champion pack is out of scope. Those found hold 90 characters and more,
# and no mod in scope holds more than 20.
MAX_CHARACTERS = 20

# The program's exit codes that leave a report: done, and ended by a fatal diagnostic.
DONE, HALTED = 0, 2
HALTED_LINE = re.compile(r'\n  "halted": (null|\d+),?\r?\n')

# How many mods a script works on at once, unless `--jobs` says.
JOBS = 12

# The brackets a count is summarized in: the top of each, and the name of the one past them.
SIZE_TOPS = [(0, "0"), (5, "1-5"), (20, "6-20"), (100, "21-100")]
SIZES = [name for _, name in SIZE_TOPS] + [">100"]


def size_of(n):
    """The bracket of a count, as `SIZES` names it."""
    return next((name for top, name in SIZE_TOPS if n <= top), SIZES[-1])


def env(name, *parts):
    """An input's default: its environment variable, else a path of this checkout."""
    return os.environ.get(name) or (os.path.join(ROOT, *parts) if parts else None)


def add_datasets(parser):
    """The arguments that say where the mods are."""
    datasets = [d for d in (os.environ.get("REFORGE_DATASETS") or "").split(os.pathsep) if d]
    parser.add_argument("--datasets", nargs="+", default=datasets, help="directories of <sha256>.fantome archives")
    parser.add_argument("--meta", default=env("REFORGE_DATASET_META"), help="the directory of mod_classes.tsv and dedup_map.tsv")


def add_program(parser):
    """The arguments a run of the program takes."""
    exe = "reforge.exe" if os.name == "nt" else "reforge"
    parser.add_argument("--reforge", default=env("REFORGE_EXE", "target", "release", exe))
    parser.add_argument("--game", default=env("REFORGE_GAME"))
    parser.add_argument("--index", default=env("REFORGE_INDEX"))
    parser.add_argument("--mimir", default=env("REFORGE_MIMIR"))
    parser.add_argument("--schema", default=env("REFORGE_SCHEMA"))
    add_jobs(parser)


def add_store(parser):
    """The argument that names the directory the program keeps archives' raw snapshots in."""
    parser.add_argument("--store", default=env("REFORGE_STORE"), help="the raw snapshots' directory (#ADR-0095)")


def add_jobs(parser):
    """The argument that says how many mods are worked on at once."""
    parser.add_argument("--jobs", type=int, default=JOBS, help=f"how many at once (default {JOBS})")


def require(parser, args, *names):
    """Ends with the usage when an input has neither an argument nor its variable."""
    for name in names:
        if not getattr(args, name):
            parser.error(f"--{name} is required (or its environment variable)")


def mods_in_scope(datasets, meta):
    """The archives in scope, SHA-256 to path, with how many mods the class table holds and
    how many of them are in scope. The first dataset holding an archive gives its path."""
    with open(os.path.join(meta, "dedup_map.tsv"), encoding="utf-8") as f:
        duplicates = {line.split("\t")[0] for line in f if line.strip()}
    with open(os.path.join(meta, "mod_classes.tsv"), encoding="utf-8") as f:
        classes = [line.rstrip("\n").split("\t") for line in f if line.strip()]
    scope = set()
    for sha, _dataset, klass, _count, characters in (row[:5] for row in classes):
        characters = [c for c in characters.split(",") if c]
        champions = not any(NOT_CHAMPION.search(c) for c in characters)
        if klass in IN_SCOPE and sha not in duplicates and len(characters) <= MAX_CHARACTERS and champions:
            scope.add(sha)
    where = {}
    for directory in datasets:
        for name in sorted(os.listdir(directory)):
            sha, extension = os.path.splitext(name)
            if extension == ".fantome" and sha in scope:
                where.setdefault(sha, os.path.join(directory, name))
    return where, len(classes), len(scope)


def listed(path):
    """The SHA-256s a list names, in its order: the reduced corpus as `reduce.py` writes it,
    or a text file of one SHA-256 per line."""
    with open(path, encoding="utf-8") as f:
        text = f.read()
    if path.endswith(".json"):
        return [mod["sha256"] for mod in json.loads(text)["mods"]]
    return text.split()


def chosen(args, where):
    """The mods a script's `--list` and `--reduced` select, of those in scope: all of them
    when neither is given. Prints how many of a list were found."""
    path = REDUCED if getattr(args, "reduced", False) else getattr(args, "list", None)
    if not path:
        return sorted(where)
    wanted = listed(path)
    found = [sha for sha in wanted if sha in where]
    print(f"list: {len(wanted)} named, {len(found)} found", flush=True)
    return found


def run_mods(shas, out, command, jobs, log, every=250):
    """Runs the program over mods, one report per mod at <out>/<sha256>.json.
    `command(sha, report)` gives the command line. A report already there is kept, so a run
    resumes. A run that ends with another code than done or halted leaves <sha256>.err and
    no report. Each mod run is one line of `log`: SHA-256, exit code, seconds. Gives the
    exit code of each mod run now and the seconds it all took."""
    os.makedirs(out, exist_ok=True)

    def one(sha):
        report = os.path.join(out, sha + ".json")
        failure = os.path.join(out, sha + ".err")
        if os.path.exists(report):
            return sha, None, 0.0
        started = time.time()
        try:
            done = subprocess.run(command(sha, report), capture_output=True, encoding="utf-8", errors="replace", timeout=1800)
            code, text = done.returncode, (done.stdout or "") + (done.stderr or "")
        except subprocess.TimeoutExpired:
            code, text = -1, "timeout\n"
        if code in (DONE, HALTED) and os.path.exists(report):
            if os.path.exists(failure):
                os.remove(failure)
        else:
            with open(failure, "w", encoding="utf-8", newline="\n") as f:
                f.write(f"exit {code}\n{text}")
            if os.path.exists(report):
                os.remove(report)
        return sha, code, time.time() - started

    started = time.time()
    exits = {}
    with concurrent.futures.ThreadPoolExecutor(jobs) as pool:
        for at, (sha, code, seconds) in enumerate(pool.map(one, shas), 1):
            if code is not None:
                exits[sha] = code
                log.write(f"{sha}\t{code}\t{seconds:.1f}\n")
            failed = code not in (None, DONE, HALTED)
            if failed:
                print(f"[{at}/{len(shas)}] {sha[:12]} exit={code}", flush=True)
            if at % every == 0:
                print(f"[{at}/{len(shas)}] {len(exits)} run, {at - len(exits)} kept, {time.time() - started:.0f}s", flush=True)
    return exits, time.time() - started


def reports(run):
    """The SHA-256s a run holds a report of, sorted."""
    return sorted(name[:-5] for name in os.listdir(run) if name.endswith(".json") and len(name) == 69)


def read(run, sha):
    """One report of a run."""
    with open(os.path.join(run, sha + ".json"), encoding="utf-8") as f:
        return json.load(f)


def halted(run, sha):
    """The code of the diagnostic that ended a mod's run, or None. The report says near its
    start whether one did, so a report of a run that was not ended is not parsed."""
    with open(os.path.join(run, sha + ".json"), encoding="utf-8") as f:
        head = f.read(1 << 16)
    found = HALTED_LINE.search(head)
    if found and found.group(1) == "null":
        return None
    report = read(run, sha)
    if report.get("halted") is None:
        return None
    return next((d["code"] for d in report["diagnostics"] if d["id"] == report["halted"]), "?")


def failure(run, sha):
    """How a mod's run that left no report ended: the first line of its <sha256>.err."""
    path = os.path.join(run, sha + ".err")
    if not os.path.exists(path):
        return "not run"
    with open(path, encoding="utf-8") as f:
        return f.readline().strip()


def over(function, items, jobs):
    """`function(item)` for each item, in the items' order, in `jobs` processes. The function
    is a module's own, so that a process can find it."""
    if jobs <= 1:
        yield from map(function, items)
        return
    with concurrent.futures.ProcessPoolExecutor(jobs) as pool:
        yield from pool.map(function, items, chunksize=8)


def write(path, text):
    """A text file with LF line ends."""
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write(text)
