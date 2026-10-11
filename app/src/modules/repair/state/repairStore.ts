import { match } from "ts-pattern";
import { create } from "zustand";

import type {
  Finding,
  FixupChanges,
  ModPeek,
  ObjectView,
  PackageView,
  PartView,
  RuleSpec,
  RunAgain,
  RunKind,
  RunSettings,
  RunView,
  StageName,
  TargetsView,
} from "@/lib/bindings.gen";
import type { IdNames } from "@/modules/findings";

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

/**
 * The advanced settings a request of this kind and these settings was run with. A setting the
 * request leaves out is the standard one, as the backend reads it.
 */
export function advancedSettings(
  kind: RunKind,
  settings: RunSettings,
  through: StageName,
): AdvancedSettings {
  return {
    kind,
    through,
    fixupsOff: settings.fixups?.off ?? STANDARD.fixupsOff,
    fixupsOn: settings.fixups?.on ?? STANDARD.fixupsOn,
    rules: settings.fixups?.rules ?? STANDARD.rules,
    checksOff: settings.checks_off ?? STANDARD.checksOff,
    keepStale: settings.keep_stale ?? STANDARD.keepStale,
    keepDds: settings.keep_dds ?? STANDARD.keepDds,
    block: settings.block ?? STANDARD.block,
    target: settings.target ?? STANDARD.target,
    animReuse: settings.anim_reuse ?? STANDARD.animReuse,
    gameEventsOnly: settings.game_events_only ?? STANDARD.gameEventsOnly,
  };
}

/**
 * A stage of the run under way, as its events tell it: `resumed` for one the run took from a
 * kept run's snapshots.
 */
export interface LiveStage {
  state: "running" | "ended" | "resumed";
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
  /** A name for each id a finding names, as the stages that make the tables end. */
  names: IdNames;
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
    names: {},
    changes: null,
    objects: [],
    targets: null,
    package: null,
  };
}

/** The run under way with what a stage gave as it ended. */
export function withPart(live: LiveRun, part: PartView): LiveRun {
  return match(part)
    .with({ part: "names" }, ({ names }) => ({ ...live, names: { ...live.names, ...names } }))
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
  /** The mod picked, read alone, shown until a run names it. */
  peek: ModPeek | null;
  /** The advanced view's settings. */
  advanced: AdvancedSettings;
  setArchive: (archive: string | null) => void;
  setPeek: (peek: ModPeek | null) => void;
  setMode: (mode: RepairMode) => void;
  setLast: (last: RunView | null) => void;
  setLive: (change: (live: LiveRun | null) => LiveRun | null) => void;
  setAdvanced: (change: Partial<AdvancedSettings>) => void;
  /** Replaces the advanced settings with a preset: `STANDARD` or `REPAIR`. */
  applyPreset: (preset: AdvancedSettings) => void;
  /**
   * Takes a kept run's archive, kind and settings as if picked and typed, in the advanced view
   * where the settings show. Nothing runs until the person presses Run.
   */
  takeRun: (again: Pick<RunAgain, "kind" | "archive" | "settings">) => void;
}

/** The run page's state, kept while the user moves between pages. */
export const useRepairStore = create<RepairState>()((set) => ({
  archive: null,
  mode: "simple",
  last: null,
  live: null,
  peek: null,
  advanced: STANDARD,
  setArchive: (archive) => set({ archive, last: null, peek: null }),
  setPeek: (peek) => set({ peek }),
  setMode: (mode) => set({ mode }),
  setLast: (last) => set({ last }),
  setLive: (change) => set((state) => ({ live: change(state.live) })),
  setAdvanced: (change) => set((state) => ({ advanced: { ...state.advanced, ...change } })),
  applyPreset: (preset) => set({ advanced: preset }),
  // A run is kept when it writes its package, so its request ran through `write`.
  takeRun: ({ kind, archive, settings }) =>
    set({
      archive,
      last: null,
      peek: null,
      mode: "advanced",
      advanced: advancedSettings(kind, settings, "write"),
    }),
}));
