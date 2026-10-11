/**
 * The reads a bin's tree asks of the backend: its open, the children of each expanded node a
 * page at a time, and the projected read of several nodes at once.
 *
 * Copied from LTK Manager's `src/modules/workshop/bin/documents/hooks/useBinDocument.ts`
 * (`useBinChildren`) and `useBinRead.ts`. Cut: the open and close pair and the reopen of an
 * evicted document, since a read here names its bytes by content id and the backend parses a
 * bin it let go again.
 */
import {
  queryOptions,
  useQueries,
  type UseQueryOptions,
  type UseQueryResult,
} from "@tanstack/react-query";
import { useCallback, useMemo } from "react";

import { type AppError, type BinRows, commands, type RunRef } from "@/lib/bindings.gen";
import { runRefKey } from "@/lib/query";
import { queryFn } from "@/utils";

import { readBatches } from "./read";
import { type LoadedChildren, mergePages, PAGE_SIZE, splitKey } from "./rows";
import type { ReadRequest } from "./valueRows";

/** One bin of a run: the run, and the id of its bytes as 64 hex digits. */
export interface BinRef {
  readonly run: RunRef;
  readonly content: string;
}

/** The key every read of one bin is kept under, under its run's. */
function binKey(bin: BinRef): readonly unknown[] {
  return [...runRefKey(bin.run), "bin", bin.content];
}

/** A bin's header and its rows at depth zero. A bin's bytes never change, so neither do they. */
export function binOpenQuery(bin: BinRef) {
  return queryOptions({
    queryKey: [...binKey(bin), "open"],
    queryFn: queryFn(() => commands.binOpen(bin.run, bin.content)),
    staleTime: Infinity,
    retry: false,
  });
}

/** One expanded node, and how many pages of it the list wants. */
export interface ChildrenRequest {
  readonly key: string;
  readonly pages: number;
}

type PageQuery = UseQueryOptions<BinRows, AppError>;

/**
 * The children of every expanded node, one query per page, merged per node.
 *
 * A page that has not answered ends the node's rows at the page before it, and the node reads
 * as pending. The map keeps its identity across a render in which no page answered.
 */
export function useBinChildren(
  bin: BinRef,
  requests: readonly ChildrenRequest[],
): ReadonlyMap<string, LoadedChildren> {
  const queries = useMemo(
    (): PageQuery[] =>
      requests.flatMap((request) => {
        const [entry, path] = splitKey(request.key);
        return Array.from({ length: request.pages }, (_, page) => ({
          queryKey: [...binKey(bin), "children", request.key, page],
          queryFn: queryFn(() =>
            commands.binChildren(bin.run, bin.content, entry, path, page * PAGE_SIZE, PAGE_SIZE),
          ),
          staleTime: Infinity,
          retry: false,
        }));
      }),
    [bin, requests],
  );
  const combine = useCallback(
    (results: UseQueryResult<BinRows, AppError>[]) => {
      const loaded = new Map<string, LoadedChildren>();
      let at = 0;
      for (const request of requests) {
        const merged = mergePages(results.slice(at, at + request.pages));
        at += request.pages;
        if (merged) loaded.set(request.key, merged);
      }
      return loaded;
    },
    [requests],
  );
  return useQueries({ queries, combine });
}

/** `requests` as one string, which is what tells one render's set from the last's. */
function signatureOf(requests: readonly ReadRequest[]): string {
  return requests.map((request) => `${request.rows}\t${request.key}`).join("\n");
}

function requestsOf(signature: string): ReadRequest[] {
  if (signature === "") return [];
  return signature.split("\n").map((line) => {
    const cut = line.indexOf("\t");
    return { rows: Number(line.slice(0, cut)), key: line.slice(cut + 1) };
  });
}

type ReadQuery = UseQueryOptions<BinRows[], AppError>;

/**
 * The rows under every requested node, by key, in as few calls as the cap allows.
 *
 * A query is keyed on its batch's paths, so a set that changes before an answer lands leaves
 * that answer behind rather than folding it in. A node whose call has not answered is absent.
 * The map keeps its identity across a render that changed nothing.
 */
export function useBinRead(
  bin: BinRef,
  requests: readonly ReadRequest[],
): ReadonlyMap<string, BinRows> {
  const signature = signatureOf(requests);
  const batches = useMemo(() => readBatches(requestsOf(signature)), [signature]);
  const queries = useMemo(
    (): ReadQuery[] =>
      batches.map((batch) => ({
        queryKey: [...binKey(bin), "read", batch.entry, batch.paths],
        queryFn: queryFn(() =>
          commands.binRead(bin.run, bin.content, batch.entry, [...batch.paths]),
        ),
        staleTime: Infinity,
        retry: false,
      })),
    [bin, batches],
  );
  const combine = useCallback(
    (results: UseQueryResult<BinRows[], AppError>[]) => {
      const loaded = new Map<string, BinRows>();
      batches.forEach((batch, at) => {
        const answer = results[at]?.data;
        if (!answer) return;
        batch.paths.forEach((path, index) => {
          const page = answer[index];
          if (page) loaded.set(`${batch.entry}:${path}`, page);
        });
      });
      return loaded;
    },
    [batches],
  );
  return useQueries({ queries, combine });
}
