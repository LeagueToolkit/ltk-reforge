import { MagnifyingGlassIcon } from "@phosphor-icons/react";
import { type KeyboardEvent, useEffect, useState } from "react";

import { Button } from "@/components";
import { m } from "@/i18n";
import type { HistoryBuilds } from "@/lib/bindings.gen";
import { twMerge } from "@/utils";

import { useHistorySuggest } from "../api/history";
import { BuildSelect } from "./parts";

/** How long the box waits after a key before asking for paths: a pause in typing. */
const SUGGEST_DELAY_MS = 250;

interface SearchBarProps {
  builds: HistoryBuilds;
  text: string;
  onText: (text: string) => void;
  at: number;
  onAt: (at: number) => void;
  /** Looks the path up; with a path given, that one in place of the box's text. */
  onLookUp: (path?: string) => void;
}

/**
 * The search: a path or its hash, with the paths the hash tables spell under the box as the
 * person types, a build, and Look up. A suggestion is picked with a click or with Enter.
 */
export function SearchBar({ builds, text, onText, at, onAt, onLookUp }: SearchBarProps) {
  const [prefix, setPrefix] = useState(text);
  const [open, setOpen] = useState(false);
  const [highlight, setHighlight] = useState(-1);
  useEffect(() => {
    const wait = setTimeout(() => setPrefix(text), SUGGEST_DELAY_MS);
    return () => clearTimeout(wait);
  }, [text]);
  const suggest = useHistorySuggest(prefix);
  const suggestions = (open && suggest.data) || [];
  const shown = suggestions.length === 1 && suggestions[0] === text ? [] : suggestions;

  const pick = (path: string) => {
    onText(path);
    setOpen(false);
    setHighlight(-1);
    onLookUp(path);
  };

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "ArrowDown" && shown.length > 0) {
      event.preventDefault();
      setHighlight((at) => Math.min(at + 1, shown.length - 1));
    } else if (event.key === "ArrowUp" && shown.length > 0) {
      event.preventDefault();
      setHighlight((at) => Math.max(at - 1, -1));
    } else if (event.key === "Escape") {
      // A search box's own Escape empties it: the path typed stays.
      event.preventDefault();
      setOpen(false);
      setHighlight(-1);
    } else if (event.key === "Enter") {
      event.preventDefault();
      if (highlight >= 0 && highlight < shown.length) {
        pick(shown[highlight]);
      } else {
        setOpen(false);
        onLookUp();
      }
    }
  };

  return (
    <div className="flex flex-wrap items-start gap-2" data-ui="HistorySearch">
      <div className="relative min-w-64 flex-1">
        <label className="flex items-center gap-2 rounded-md border border-surface-600 bg-surface-950/40 px-2 focus-within:outline-2 focus-within:outline-accent-500">
          <MagnifyingGlassIcon className="h-3.5 w-3.5 shrink-0 text-surface-400" />
          <input
            type="search"
            role="combobox"
            aria-expanded={shown.length > 0}
            aria-controls="history-suggestions"
            aria-label={m.history_search_label()}
            placeholder={m.history_search_placeholder()}
            value={text}
            spellCheck={false}
            onChange={(event) => {
              onText(event.target.value);
              setOpen(true);
              setHighlight(-1);
            }}
            onFocus={() => setOpen(true)}
            onBlur={() => setOpen(false)}
            onKeyDown={onKeyDown}
            className="h-9 min-w-0 flex-1 bg-transparent font-mono text-xs text-surface-100 outline-none placeholder:font-sans placeholder:text-surface-500"
          />
        </label>
        {shown.length > 0 && (
          <ul
            id="history-suggestions"
            role="listbox"
            aria-label={m.history_suggestions_label()}
            className="absolute inset-x-0 top-full z-40 mt-1 max-h-80 overflow-y-auto rounded-lg border border-surface-700 bg-surface-800 p-1 shadow-lg"
          >
            {shown.map((path, index) => (
              <li
                key={path}
                role="option"
                aria-selected={index === highlight}
                // Before the box's blur, which closes the list.
                onMouseDown={(event) => {
                  event.preventDefault();
                  pick(path);
                }}
                onMouseEnter={() => setHighlight(index)}
                className={twMerge(
                  "cursor-pointer truncate rounded-md px-2 py-1 font-mono text-xs text-surface-200",
                  index === highlight && "bg-surface-veil text-surface-50",
                )}
              >
                {path}
              </li>
            ))}
          </ul>
        )}
      </div>
      <BuildSelect
        builds={builds}
        value={at}
        onChange={(picked) => picked != null && onAt(picked)}
        label={m.history_build_label()}
        className="h-9"
      />
      <Button
        variant="filled"
        left={<MagnifyingGlassIcon weight="bold" />}
        disabled={text.trim() === ""}
        onClick={() => onLookUp()}
      >
        {m.history_lookup_action()}
      </Button>
    </div>
  );
}
