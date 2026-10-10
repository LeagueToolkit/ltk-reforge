/**
 * The lines of "Refuse the write on", as the library's filter reads each: a code or an area,
 * or `key=value`. A checkbox owns the one line that is exactly its term; every other line is
 * the raw field's alone.
 */

import type { StageName } from "@/lib/bindings.gen";

/** The severities a checkbox offers, each meaning that severity or worse. */
export const SEVERITIES = ["error", "warning", "note"] as const;

/** A group of areas: the stage that raises them, or the run's inputs, which no stage owns. */
export interface AreaGroup {
  stage: StageName | "inputs";
  areas: readonly string[];
}

/**
 * The areas a code is in, under the stage `docs/design/CODES.md` lists them with. An area a
 * later stage also raises (`audio`, `catalog`) sits with the stage that raises most of it.
 */
export const AREA_GROUPS: readonly AreaGroup[] = [
  { stage: "read", areas: ["archive", "inventory"] },
  { stage: "load", areas: ["wad", "character", "file", "bin", "rework"] },
  { stage: "fix", areas: ["fixup", "catalog"] },
  { stage: "facts", areas: ["copies", "origin", "rig"] },
  { stage: "decide", areas: ["target"] },
  { stage: "plan", areas: ["unit", "object", "ref", "audio", "hud"] },
  { stage: "build", areas: ["package"] },
  { stage: "check", areas: ["check"] },
  { stage: "write", areas: ["write"] },
  { stage: "inputs", areas: ["install", "names", "history", "schema"] },
];

/** Every area a checkbox offers, in the groups' order. */
export const AREAS: readonly string[] = AREA_GROUPS.flatMap((group) => group.areas);

/** The line a severity checkbox owns. */
export function severityLine(severity: string): string {
  return `severity=${severity}`;
}

/** Whether a line is the term, allowing the spaces and the `.*` the filter allows. */
export function lineIs(line: string, term: string): boolean {
  const bare = line.replace(/\s+/g, "");
  return bare === term || bare === `${term}.*`;
}

/** Whether any line is the term. */
export function hasLine(lines: readonly string[], term: string): boolean {
  return lines.some((line) => lineIs(line, term));
}

/** The lines with the term's line added when `on`, or every line of it taken out. */
export function withLine(lines: readonly string[], term: string, on: boolean): string[] {
  const rest = lines.filter((line) => !lineIs(line, term));
  return on ? [...rest, term] : rest;
}
