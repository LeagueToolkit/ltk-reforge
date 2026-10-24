import { queryOptions, useQuery } from "@tanstack/react-query";

import { m } from "@/i18n";
import { type BatchJobs, commands } from "@/lib/bindings.gen";
import { unwrap } from "@/utils";

/** How many archives a batch runs at once when none is chosen: the app's choice. */
export const AUTOMATIC_JOBS = 0;

/** The machine's choices of how many archives run at once (#ADR-0108); they do not change. */
export const batchJobsOptions = queryOptions({
  queryKey: ["batch-jobs"],
  queryFn: async () => unwrap(await commands.batchJobs()),
  staleTime: Infinity,
});

export function useBatchJobs() {
  return useQuery(batchJobsOptions);
}

/** The choices the setting offers: the app's, then every count up to the most allowed. */
export function jobsChoices(jobs: BatchJobs): number[] {
  return [AUTOMATIC_JOBS, ...Array.from({ length: jobs.max }, (_, n) => n + 1)];
}

/** A choice's label, which says what the app's choice comes to on this machine. */
export function jobsLabel(choice: number, jobs: BatchJobs | undefined): string {
  if (choice === AUTOMATIC_JOBS) {
    return jobs ? m.jobs_automatic_label({ count: jobs.automatic }) : m.jobs_automatic_unknown();
  }
  return String(choice);
}
