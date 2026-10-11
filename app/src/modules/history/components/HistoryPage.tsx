import { Link } from "@tanstack/react-router";
import { useEffect } from "react";

import { FailureNote, Spinner } from "@/components";
import { m } from "@/i18n";
import type { HistoryBuilds } from "@/lib/bindings.gen";
import { useInputs } from "@/modules/inputs";

import { useHistoryBuilds, useHistoryLookup } from "../api/history";
import { useHistoryStore } from "../state/historyStore";
import { LookupResult } from "./LookupResult";
import { ObjectSection } from "./ObjectSection";
import { SearchBar } from "./SearchBar";

/**
 * The search and what it found, over the builds of the index. What was asked lives in the
 * page's store and the answers in the query cache, so leaving the page and coming back shows
 * the same.
 */
function HistoryExplorer({ builds }: { builds: HistoryBuilds }) {
  const state = useHistoryStore();
  const { head, text, at, asked, bin, entry, object, start, seed, lookUp: ask } = state;
  const lookup = useHistoryLookup(asked?.query ?? null, asked?.at ?? null);

  // Another index's head starts the page over.
  useEffect(() => start(builds.head), [builds.head, start]);

  // A path asked about fills the object's bin when it is a bin, once per lookup.
  const found = lookup.data;
  useEffect(() => {
    if (found && asked) {
      seed(asked, found.path, found.bin != null);
    }
  }, [found, asked, seed]);

  if (head !== builds.head) {
    return null;
  }

  const lookUp = (path: string | undefined, atBuild: number) => {
    const query = (path ?? text).trim();
    if (query !== "") {
      ask(query, atBuild);
    }
  };

  return (
    <div className="flex flex-col gap-6">
      <SearchBar
        builds={builds}
        text={text}
        onText={state.setText}
        at={at ?? builds.head}
        onAt={state.setAt}
        onLookUp={(path) => lookUp(path, at ?? builds.head)}
      />
      {lookup.isFetching && !found && <Spinner />}
      {lookup.isError && <FailureNote failure={lookup.error} />}
      {found && (
        <>
          <LookupResult
            view={found}
            onLookUp={(path) => lookUp(path, found.at.position)}
            onOpenObject={state.openObject}
          />
          <ObjectSection
            at={found.at}
            builds={builds}
            bin={bin}
            entry={entry}
            onBin={state.setBin}
            onEntry={state.setEntry}
            asked={object}
            onShow={state.showObject}
            other={state.other}
            onOther={state.setOther}
          />
        </>
      )}
    </div>
  );
}

/**
 * The history index explored (#ADR-0091): a path searched, hashes spelled through the hash
 * tables; the builds and the WADs that held it; a bin's entries; an object at any build as
 * text, and compared between two builds. A renderer of the library's answers (#ADR-0046).
 */
export function HistoryPage() {
  const inputs = useInputs();
  const hasIndex = inputs.data?.history.dir != null;
  const builds = useHistoryBuilds(hasIndex);
  return (
    <div className="flex flex-col gap-6" data-ui="HistoryPage">
      <div className="flex flex-col gap-1">
        <h1 className="font-display text-xl font-semibold text-surface-50">
          {m.history_page_title()}
        </h1>
        <p className="text-sm text-surface-400 select-none">{m.history_page_description()}</p>
      </div>
      {inputs.isPending && <Spinner />}
      {inputs.isError && <FailureNote failure={inputs.error} />}
      {inputs.isSuccess && !hasIndex && (
        <p className="text-sm text-surface-300 select-none" data-ui="HistoryNoIndex">
          {m.history_no_index()}{" "}
          <Link to="/inputs" className="text-accent-300 underline-offset-2 hover:underline">
            {m.history_no_index_link()}
          </Link>
        </p>
      )}
      {hasIndex && builds.isPending && <Spinner />}
      {builds.isError && <FailureNote failure={builds.error} />}
      {builds.data && <HistoryExplorer key={builds.data.head} builds={builds.data} />}
    </div>
  );
}
