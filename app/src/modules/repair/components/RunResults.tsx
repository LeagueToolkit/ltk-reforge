import type { Finding, FixupChanges, ObjectView, RunView, TargetsView } from "@/lib/bindings.gen";
import { ChangesExplorer } from "@/modules/changes";
import type { IdNames } from "@/modules/findings";

import type { LiveRun } from "../state/repairStore";
import { DecisionsView } from "./DecisionsView";
import { RunFindings } from "./RunFindings";

export interface RunResultsProps {
  findings: Finding[];
  /** What the fixups changed. Null until `fix` ended. */
  changes: FixupChanges[] | null;
  objects: ObjectView[];
  /** The decisions of a port. Null until `decide` ended. */
  targets: TargetsView | null;
  /** A name for each id a finding's fields can hold. */
  names: IdNames;
}

/**
 * The results of the run under way, as far as it got. Its names are the changed objects'
 * alone: the rest come with the run's end.
 */
export function liveResults(live: LiveRun): RunResultsProps {
  const { findings, changes, objects, targets } = live;
  const names = Object.fromEntries(objects.map((object) => [object.id, object.entry]));
  return { findings, changes, objects, targets, names };
}

/** The results of a run that ended. Its changes are none when `fix` did not run. */
export function endedResults(last: RunView): RunResultsProps {
  const fixed = last.stages.some((stage) => stage.stage === "fix" && stage.state === "ran");
  const { findings, objects, targets, names } = last;
  return { findings, changes: fixed ? last.changes : null, objects, targets, names };
}

/** What the results show: the run under way, else the last run. None before the first. */
export function resultsOf(live: LiveRun | null, last: RunView | null): RunResultsProps | null {
  if (live) {
    return liveResults(live);
  }
  return last && endedResults(last);
}

/**
 * What a run found and did: its findings, a port's decisions, and every value the fixups
 * changed. The same while the run goes, each part shown as its stage ends, and after it.
 */
export function RunResults({ findings, changes, objects, targets, names }: RunResultsProps) {
  return (
    <>
      <RunFindings findings={findings} changes={changes} objects={objects} names={names} />
      {targets && <DecisionsView targets={targets} />}
      {changes && <ChangesExplorer changes={changes} objects={objects} findings={findings} />}
    </>
  );
}
