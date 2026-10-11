import { Link } from "@tanstack/react-router";
import { useEffect, useState } from "react";

import { FailureNote, Spinner } from "@/components";
import { m } from "@/i18n";
import type { HistoryBuilds } from "@/lib/bindings.gen";
import { useInputs } from "@/modules/inputs";

import { useHistoryBuilds, useHistoryLookup } from "../api/history";
import { LookupResult } from "./LookupResult";
import { type ObjectAsked, ObjectSection } from "./ObjectSection";
import { SearchBar } from "./SearchBar";

/** A path asked about, at a build. */
interface Asked {
  query: string;
  at: number;
}

/** The search and what it found, over the builds of the index. */
function HistoryExplorer({ builds }: { builds: HistoryBuilds }) {
  const [text, setText] = useState("");
  const [at, setAt] = useState(builds.head);
  const [asked, setAsked] = useState<Asked | null>(null);
  const [bin, setBin] = useState("");
  const [entry, setEntry] = useState("");
  const [object, setObject] = useState<ObjectAsked | null>(null);
  const lookup = useHistoryLookup(asked?.query ?? null, asked?.at ?? null);

  // A new path asked about starts the object over, from that path when it is a bin.
  const found = lookup.data;
  useEffect(() => {
    if (found) {
      setBin(found.bin ? found.path : "");
      setEntry("");
      setObject(null);
    }
  }, [found]);

  const lookUp = (path: string | undefined, atBuild: number) => {
    const query = (path ?? text).trim();
    if (query !== "") {
      setText(query);
      setAt(atBuild);
      setAsked({ query, at: atBuild });
    }
  };
  const openObject = (objectBin: string, objectEntry: string) => {
    setBin(objectBin);
    setEntry(objectEntry);
    setObject({ bin: objectBin, entry: objectEntry });
  };

  return (
    <div className="flex flex-col gap-6">
      <SearchBar
        builds={builds}
        text={text}
        onText={setText}
        at={at}
        onAt={setAt}
        onLookUp={(path) => lookUp(path, at)}
      />
      {lookup.isFetching && !found && <Spinner />}
      {lookup.isError && <FailureNote failure={lookup.error} />}
      {found && (
        <>
          <LookupResult
            view={found}
            onLookUp={(path) => lookUp(path, found.at.position)}
            onOpenObject={openObject}
          />
          <ObjectSection
            at={found.at}
            builds={builds}
            bin={bin}
            entry={entry}
            onBin={setBin}
            onEntry={setEntry}
            asked={object}
            onShow={() => setObject({ bin: bin.trim(), entry: entry.trim() })}
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
