import { describe, expect, it } from "vitest";

import type { FixupChanges, RunView, TargetsView } from "@/lib/bindings.gen";

import { endedResults } from "../components/RunResults";
import { startedRun, withPart } from "./repairStore";

const changes: FixupChanges[] = [
  { fixup: "retype", changed: 0, removed: 0, lost: 0, unconvertible: 0, changes: [] },
];

const targets: TargetsView = {
  policy: { target: "auto", anim_reuse: "own-rig" },
  decisions: [],
};

function ended(over: Partial<RunView>): RunView {
  return {
    stages: [],
    meta: null,
    halted: null,
    findings: [],
    changes,
    objects: [],
    targets: null,
    package: null,
    written: null,
    refused: false,
    fault: null,
    millis: 0,
    log: [],
    names: {},
    ...over,
  };
}

describe("the run under way", () => {
  it("takes each stage's part as it ends, and keeps what it had", () => {
    const fixed = withPart(startedRun(), { part: "changes", changes, objects: [] });
    expect(fixed.changes).toBe(changes);
    expect(fixed.targets).toBeNull();

    const decided = withPart(fixed, { part: "targets", targets });
    expect(decided.targets).toBe(targets);
    expect(decided.changes).toBe(changes);
  });

  it("shows no changes for a run that did not reach fix, and a run's that did", () => {
    const halted = ended({
      stages: [
        { stage: "read", state: "ran", millis: 1 },
        { stage: "load", state: "halted", millis: 1 },
        { stage: "fix", state: "not-run", millis: null },
      ],
    });
    expect(endedResults(halted).changes).toBeNull();

    const fixed = ended({ stages: [{ stage: "fix", state: "ran", millis: 3 }] });
    expect(endedResults(fixed).changes).toBe(changes);
  });
});
