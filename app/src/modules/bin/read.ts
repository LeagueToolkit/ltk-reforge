/**
 * The projected read cut into calls under the backend's cap.
 *
 * Copied from LTK Manager's `src/modules/workshop/bin/documents/hooks/useBinRead.ts`
 * (`readBatches`).
 */
import { PAGE_SIZE, splitKey } from "./rows";
import type { ReadRequest } from "./valueRows";

/**
 * How many rows one projected read answers, past which the backend refuses it.
 *
 * `READ_ROW_CAP` in `crates/reforge-app/src/bin_rows/types.rs`. A caller batches under it
 * here, so the refusal is the guard rather than the path a reader meets.
 */
export const READ_ROW_CAP = 2000;

/** One call: the entry the paths are under, and the paths in the order asked. */
export interface ReadBatch {
  readonly entry: string;
  readonly paths: readonly string[];
}

/**
 * `requests` cut into calls, each over one entry and each under the row cap.
 *
 * Sorted by key, which groups an entry's paths together and drops a key asked for twice. A path
 * answers one page at most, so its cost is the smaller of the two.
 */
export function readBatches(requests: readonly ReadRequest[]): ReadBatch[] {
  const wanted = new Map<string, number>();
  for (const request of requests) wanted.set(request.key, request.rows);

  const open: { entry: string; paths: string[]; rows: number }[] = [];
  for (const key of [...wanted.keys()].sort()) {
    const [entry, path] = splitKey(key);
    const cost = Math.min(wanted.get(key) ?? 0, PAGE_SIZE);
    const last = open.at(-1);
    if (last === undefined || last.entry !== entry || last.rows + cost > READ_ROW_CAP) {
      open.push({ entry, paths: [path], rows: cost });
      continue;
    }
    last.paths.push(path);
    last.rows += cost;
  }

  return open.map(({ entry, paths }) => ({ entry, paths }));
}
