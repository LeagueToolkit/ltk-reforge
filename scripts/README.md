# scripts

## corpus

The corpus tooling (#ADR-0044, #ADR-0045): a run of reforge over the mods of the datasets, the
comparison of two runs, the edge cases a run shows, and the reduced corpus. Each script's
header states its inputs and outputs.

| script | what it is for |
|---|---|
| `corpus/corpus.py` | what the others share: the mods in scope (#ADR-0033), where each archive is, the runner, reading a report. Its header lists the inputs and their environment variables |
| `corpus/run.py` | runs one command (`port --no-write`, `repair --no-write --again`, `inspect`, `load`) over the mods in scope, a list of SHA-256s or the reduced corpus: one report per mod, a log, and a summary of the runs a fatal diagnostic ended. A report already there is kept, so a run resumes |
| `corpus/compare.py` | compares two runs mod by mod and section by section: one line per difference in `differences.tsv`, and the counts by section and by diagnostic code in `summary.txt`. Exits non-zero on a difference. `--expect` takes the kinds of difference a change under work is meant to make |
| `corpus/cases.py` | the edge cases the reports of runs show, each with how many mods show it and up to five of them, the cases the program declares that no mod shows, and the places of a report its table does not know. Writes the generated tables of `docs/design/CASES.md` again (`--doc`) |
| `corpus/reduce.py` | the reduced corpus: the fewest mods that together show every case, as `corpus/reduced.json` |
| `corpus/reduced.json` | the reduced corpus of the baselines, for `run.py --reduced` and `compare.py --reduced` |

Every input is an argument or an environment variable. A mod is named by its SHA-256. No
archive is extracted: reforge reads it in memory, and a run writes reports only.

### The baseline

A baseline is a run directory: the reports of one command over every mod in scope, made by
one build of reforge. There is one for `port` and one for `repair`. The reports are large and
are kept outside the repository, in a directory named for the command and numbered
(`baseline-port-1`, `baseline-repair-1`). The repository holds what is derived from them:
`corpus/reduced.json`, the tables of `docs/design/CASES.md`, and the numbers in
`docs/design/RUNS.md`.

A change is measured against the baseline:

1. Run the reduced corpus with the changed build and compare it with the baseline.

   ```
   run.py port --reduced --out <new>
   compare.py <baseline-port> <new> --out <new>/compare
   ```

   The same for `repair`. A run that changes nothing reports no difference. Two runs of one
   build compared with each other report none either: the reports hold no clock.
2. Each difference is meant, or it is a fault. For a change that is meant to differ, write
   the kinds of difference it makes in a file and pass it as `--expect`, so that the others
   stand out. The file belongs to the change under work and is not kept.
3. Run the mods the change affects, by a list of their SHA-256s (`run.py --list`), before any
   run of the datasets.

The baseline is replaced when a change that is meant to differ is done:

1. Run the command over the datasets into the next numbered directory, and compare it with
   the baseline it replaces. Every difference is one the change means.
2. Derive the cases and the reduced corpus again, from the new baselines of both commands:

   ```
   cases.py --out <dir> --doc docs/design/CASES.md --dated <date> port=<baseline-port> repair=<baseline-repair>
   reduce.py --cases <dir>/mods.json
   ```

   `cases.py` prints the places of a report and the values its table does not know. Add them
   to the table in `cases.py` before the tables are taken as final.
3. Run the reduced corpus and compare it with the new baseline: no difference.
4. Record the run in `docs/design/RUNS.md`, dated: mods, runs ended by a fatal diagnostic,
   cases seen and not seen, the size of the reduced corpus, the time.
5. Delete the baseline that was replaced.

The reduced corpus is also made again when an edge case is added (#ADR-0045): a new diagnostic
code, a new value of a closed set, or a new rule in the table of `cases.py`.
