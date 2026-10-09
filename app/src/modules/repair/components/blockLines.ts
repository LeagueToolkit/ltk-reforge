/**
 * The lines of "Refuse the write on", as the library's filter reads each: a code or an area,
 * or `key=value`. A checkbox owns the one line that is exactly its term; every other line is
 * the raw field's alone.
 */

/** The severities a checkbox offers, each meaning that severity or worse. */
export const SEVERITIES = ["error", "warning", "note"] as const;

/** The areas a code is in, in the order `docs/design/CODES.md` lists them. */
export const AREAS = [
  "archive",
  "inventory",
  "wad",
  "character",
  "file",
  "bin",
  "rework",
  "fixup",
  "catalog",
  "copies",
  "origin",
  "rig",
  "target",
  "unit",
  "object",
  "ref",
  "audio",
  "hud",
  "package",
  "check",
  "write",
  "install",
  "names",
  "history",
  "schema",
] as const;

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
