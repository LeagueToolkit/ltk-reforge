import { match } from "ts-pattern";
import { create } from "zustand";

import type {
  Finding,
  FixupChanges,
  LogLine,
  ObjectView,
  PackageView,
  PartView,
  RuleSpec,
  RunKind,
  RunSettings,
  RunView,
  StageName,
  TargetsView,
} from "@/lib/bindings.gen";

/** Which view of the run the page shows (#ADR-0050). */
export type RepairMode = "simple" | "advanced";

/** The settings of the advanced view, as the run request takes them. */
export interface AdvancedSettings {
  kind: RunKind;
  through: StageName;
  fixupsOff: string[];
  fixupsOn: string[];
  rules: RuleSpec[];
  checksOff: string[];
  keepStale: boolean;
  keepDds: boolean;
  block: string[];
  target: string;
  animReuse: string;
  gameEventsOnly: boolean;
}

/** A port with the standard set and the default policy: what the simple view runs (#ADR-0052). */
export const STANDARD: AdvancedSettings = {
  kind: "port",
  through: "write",
  fixupsOff: [],
  fixupsOn: [],
  rules: [],
  checksOff: [],
  keepStale: false,
  keepDds: false,
  block: [],
  target: "auto",
  animReuse: "own-rig",
  gameEventsOnly: false,
};

/** A repair with the standard set: the mod at its own slots and paths (#ADR-0011, #ADR-0052). */
export const REPAIR: AdvancedSettings = { ...STANDARD, kind: "repair" };

/** A list with an item added when absent and taken out when present. */
export function toggled(list: string[], item: string): string[] {
  return list.includes(item) ? list.filter((x) => x !== item) : [...list, item].sort();
}

/** The advanced settings as the run request takes them. */
export function runSettings(settings: AdvancedSettings): RunSettings {
  return {
    fixups: { off: settings.fixupsOff, on: settings.fixupsOn, rules: settings.rules },
    checks_off: settings.checksOff,
    keep_stale: settings.keepStale,
    keep_dds: settings.keepDds,
    block: settings.block.filter((line) => line.trim() !== ""),
    target: settings.target,
    anim_reuse: settings.animReuse,
    game_events_only: settings.gameEventsOnly,
  };
}

/** A stage of the run under way, as its events tell it. */
export interface LiveStage {
  state: "running" | "ended";
  millis?: number;
}

/**
 * The run under way, from its events: the stages started and ended, the findings and details
 * as they come, and what each stage gave as it ended.
 */
export interface LiveRun {
  started: number;
  stages: Partial<Record<StageName, LiveStage>>;
  findings: Finding[];
  log: LogLine[];
  /** What the fixups changed, once `fix` ended. */
  changes: FixupChanges[] | null;
  objects: ObjectView[];
  /** The decisions, once `decide` ended. */
  targets: TargetsView | null;
  /** The package, once `build` ended. */
  package: PackageView | null;
}

/** A run with nothing heard of it yet, started now. */
export function startedRun(): LiveRun {
  return {
    started: Date.now(),
    stages: {},
    findings: [],
    log: [],
    changes: null,
    objects: [],
    targets: null,
    package: null,
  };
}

/** The run under way with what a stage gave as it ended. */
export function withPart(live: LiveRun, part: PartView): LiveRun {
  return match(part)
    .with({ part: "changes" }, ({ changes, objects }) => ({ ...live, changes, objects }))
    .with({ part: "targets" }, ({ targets }) => ({ ...live, targets }))
    .with({ part: "package" }, ({ package: built }) => ({ ...live, package: built }))
    .exhaustive();
}

interface RepairState {
  /** The mod archive picked. */
  archive: string | null;
  /** The view shown. */
  mode: RepairMode;
  /** The last run of the archive, as the backend answered it. */
  last: RunView | null;
  /** The run under way, while one is. */
  live: LiveRun | null;
  /** The advanced view's settings. */
  advanced: AdvancedSettings;
  setArchive: (archive: string | null) => void;
  setMode: (mode: RepairMode) => void;
  setLast: (last: RunView | null) => void;
  setLive: (change: (live: LiveRun | null) => LiveRun | null) => void;
  setAdvanced: (change: Partial<AdvancedSettings>) => void;
  /** Replaces the advanced settings with a preset: `STANDARD` or `REPAIR`. */
  applyPreset: (preset: AdvancedSettings) => void;
}

/** The run page's state, kept while the user moves between pages. */
export const useRepairStore = create<RepairState>()((set) => ({
  archive: null,
  mode: "simple",
  last: null,
  live: null,
  advanced: STANDARD,
  setArchive: (archive) => set({ archive, last: null }),
  setMode: (mode) => set({ mode }),
  setLast: (last) => set({ last }),
  setLive: (change) => set((state) => ({ live: change(state.live) })),
  setAdvanced: (change) => set((state) => ({ advanced: { ...state.advanced, ...change } })),
  applyPreset: (preset) => set({ advanced: preset }),
}));
