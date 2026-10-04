import type { Finding, Severity } from "@/lib/bindings.gen";

const RANK: Record<Severity, number> = { fatal: 0, error: 1, warning: 2, note: 3 };

/** Findings with the worst first, in the order raised within a severity. */
export function worstFirst(findings: Finding[]): Finding[] {
  return findings
    .map((finding, at) => ({ finding, at }))
    .sort((a, b) => RANK[a.finding.severity] - RANK[b.finding.severity] || a.at - b.at)
    .map(({ finding }) => finding);
}

/** How many findings there are of each severity. */
export function countBySeverity(findings: Finding[]): Record<Severity, number> {
  const counts: Record<Severity, number> = { fatal: 0, error: 0, warning: 0, note: 0 };
  for (const finding of findings) {
    counts[finding.severity] += 1;
  }
  return counts;
}

/** Whether a finding costs the output something: an error or a fatal finding. */
export function isError(finding: Finding): boolean {
  return finding.severity === "error" || finding.severity === "fatal";
}
