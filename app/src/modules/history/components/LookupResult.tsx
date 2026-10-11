import { CaretRightIcon, MagnifyingGlassIcon } from "@phosphor-icons/react";
import { useState } from "react";

import { SectionCard, Tooltip } from "@/components";
import { m } from "@/i18n";
import type { BinLookup, EntryView, LookupView } from "@/lib/bindings.gen";
import { twMerge } from "@/utils";

import { cellClass, headClass, lifeShort, Row, Table } from "./parts";

/** How many hex digits of a content id a table shows; the whole is on hover. */
const SHORT_CONTENT = 12;

/** The path, its hash and kind, and when the game held a file there. */
function LookupHeader({ view }: { view: LookupView }) {
  const life = view.life
    ? m.history_life_text({
        first: view.life.first.version,
        last: view.life.last.version,
        count: view.life.builds,
      })
    : m.history_life_never();
  return (
    <div className="flex flex-col gap-1" data-ui="HistoryHeader">
      <p className="font-mono text-sm break-all text-surface-50">{view.path}</p>
      <p className="flex flex-wrap gap-x-4 gap-y-1 text-xs text-surface-400">
        <span>
          {m.history_hash_label()} <span className="font-mono text-surface-300">{view.hash}</span>
        </span>
        {view.kind != null && (
          <span>
            {m.history_kind_label()} <span className="font-mono text-surface-300">{view.kind}</span>
          </span>
        )}
        <span className={twMerge(view.life ? "text-surface-200" : "text-warning-text")}>
          {life}
        </span>
      </p>
    </div>
  );
}

/** The WADs that held a file at the path at the build. */
function HeldIn({ view }: { view: LookupView }) {
  return (
    <SectionCard
      title={m.history_held_title()}
      description={m.history_held_description({ version: view.at.version })}
    >
      {view.placed.length === 0 ? (
        <p className="text-sm text-surface-400 select-none">{m.history_held_none()}</p>
      ) : (
        <Table
          head={
            <>
              <th className={headClass}>{m.history_table_wad()}</th>
              <th className={headClass}>{m.history_table_content()}</th>
              <th className={headClass}>{m.history_table_life()}</th>
            </>
          }
        >
          {view.placed.map((held) => (
            <Row key={`${held.wad}/${held.content}`}>
              <td className={twMerge(cellClass, "font-mono text-xs break-all text-surface-100")}>
                {held.wad}
              </td>
              <td className={cellClass}>
                <Tooltip content={<span className="font-mono text-xs">{held.content}</span>}>
                  <span className="font-mono text-xs text-surface-300">
                    {held.content.slice(0, SHORT_CONTENT)}
                  </span>
                </Tooltip>
              </td>
              <td className={twMerge(cellClass, "text-xs whitespace-nowrap text-surface-300")}>
                {lifeShort(held.life)}
              </td>
            </Row>
          ))}
        </Table>
      )}
    </SectionCard>
  );
}

interface NamedByProps {
  view: LookupView;
  onOpenObject: (bin: string, entry: string) => void;
}

/** The game objects that named the path, each a link to the object at the build. */
function NamedBy({ view, onOpenObject }: NamedByProps) {
  return (
    <SectionCard title={m.history_named_title()} description={m.history_named_description()}>
      {view.named_by.length === 0 ? (
        <p className="text-sm text-surface-400 select-none">{m.history_named_none()}</p>
      ) : (
        <Table
          head={
            <>
              <th className={headClass}>{m.history_table_object()}</th>
              <th className={headClass}>{m.history_table_life()}</th>
            </>
          }
        >
          {view.named_by.map((named) => (
            <Row key={`${named.bin_hash}/${named.entry_hash}/${named.life.first.position}`}>
              <td className={cellClass}>
                <button
                  type="button"
                  onClick={() => onOpenObject(named.bin, named.entry)}
                  className="cursor-pointer text-left font-mono text-xs break-all text-accent-300 underline-offset-2 hover:underline"
                >
                  <span className="text-surface-300">{named.bin}</span>
                  <CaretRightIcon
                    weight="bold"
                    className="mx-1 inline h-3 w-3 text-surface-500"
                    aria-hidden
                  />
                  {named.entry}
                </button>
              </td>
              <td className={twMerge(cellClass, "text-xs whitespace-nowrap text-surface-300")}>
                {lifeShort(named.life)}
              </td>
            </Row>
          ))}
        </Table>
      )}
      {view.named_by_capped && (
        <p className="text-xs text-warning-text select-none">
          {m.history_named_capped({ count: view.named_by.length })}
        </p>
      )}
    </SectionCard>
  );
}

function entryMatches(entry: EntryView, filter: string): boolean {
  const lower = filter.trim().toLowerCase();
  return (
    lower === "" ||
    entry.entry.toLowerCase().includes(lower) ||
    entry.hash.includes(lower) ||
    entry.class.toLowerCase().includes(lower)
  );
}

interface BinSectionProps {
  path: string;
  version: string;
  bin: BinLookup;
  onLookUp: (path: string) => void;
  onOpenObject: (bin: string, entry: string) => void;
}

/** The bin at the path at the build: its WAD, its links and its entries. */
function BinSection({ path, version, bin, onLookUp, onOpenObject }: BinSectionProps) {
  const [filter, setFilter] = useState("");
  const shown = bin.entries.filter((entry) => entryMatches(entry, filter));
  return (
    <SectionCard title={m.history_bin_title()} description={m.history_bin_description({ version })}>
      <p className="flex flex-wrap gap-x-4 gap-y-1 text-xs text-surface-400">
        <span>
          {m.history_bin_wad_label()}{" "}
          <span className="font-mono break-all text-surface-200">{bin.wad}</span>
        </span>
        <span className="text-surface-300">{lifeShort(bin.life)}</span>
      </p>
      <div className="flex flex-col gap-1.5">
        <p className="text-xs font-medium text-surface-300 select-none">
          {m.history_bin_links_label()}
        </p>
        {bin.links.length === 0 ? (
          <p className="text-xs text-surface-400 select-none">{m.history_bin_no_links()}</p>
        ) : (
          <div className="flex flex-wrap gap-1.5">
            {bin.links.map((link) => (
              <button
                key={link.hash}
                type="button"
                onClick={() => onLookUp(link.path)}
                // A path of a linked bin runs to several lines, so the chip is a box.
                className="inline-flex cursor-pointer items-start gap-1 rounded-md border border-surface-700 bg-surface-800 px-2 py-0.5 text-left font-mono text-xs break-all text-surface-200 hover:border-accent-500/60 hover:text-surface-50"
              >
                <MagnifyingGlassIcon className="mt-0.5 h-3 w-3 shrink-0 text-surface-400" />
                {link.path}
              </button>
            ))}
          </div>
        )}
      </div>
      <label className="flex items-center gap-2 rounded-md border border-surface-600 bg-surface-950/40 px-2 focus-within:outline-2 focus-within:outline-accent-500">
        <MagnifyingGlassIcon className="h-3.5 w-3.5 shrink-0 text-surface-400" />
        <input
          type="search"
          value={filter}
          onChange={(event) => setFilter(event.target.value)}
          placeholder={m.history_entries_filter_placeholder()}
          aria-label={m.history_entries_filter_label()}
          className="h-8 min-w-0 flex-1 bg-transparent text-xs text-surface-100 outline-none placeholder:text-surface-500"
        />
      </label>
      {shown.length === 0 ? (
        <p className="text-sm text-surface-400 select-none">{m.history_entries_none_match()}</p>
      ) : (
        <Table
          head={
            <>
              <th className={headClass}>{m.history_table_entry()}</th>
              <th className={headClass}>{m.history_table_hash()}</th>
              <th className={headClass}>{m.history_table_class()}</th>
            </>
          }
        >
          {shown.map((entry) => (
            <Row key={entry.hash}>
              <td className={cellClass}>
                <button
                  type="button"
                  onClick={() => onOpenObject(path, entry.entry)}
                  className="cursor-pointer text-left font-mono text-xs break-all text-accent-300 underline-offset-2 hover:underline"
                >
                  {entry.entry}
                </button>
              </td>
              <td className={twMerge(cellClass, "font-mono text-xs text-surface-400")}>
                {entry.hash}
              </td>
              <td className={twMerge(cellClass, "font-mono text-xs break-all text-surface-300")}>
                {entry.class}
              </td>
            </Row>
          ))}
        </Table>
      )}
    </SectionCard>
  );
}

interface LookupResultProps {
  view: LookupView;
  /** Looks another path up at the same build. */
  onLookUp: (path: string) => void;
  /** Opens an object of a bin at the build. */
  onOpenObject: (bin: string, entry: string) => void;
}

/** A path looked up: its header, the WADs that held it, the objects that named it, its bin. */
export function LookupResult({ view, onLookUp, onOpenObject }: LookupResultProps) {
  return (
    <div className="flex flex-col gap-6" data-ui="HistoryResult">
      <LookupHeader view={view} />
      <HeldIn view={view} />
      <NamedBy view={view} onOpenObject={onOpenObject} />
      {view.bin && (
        <BinSection
          path={view.path}
          version={view.at.version}
          bin={view.bin}
          onLookUp={onLookUp}
          onOpenObject={onOpenObject}
        />
      )}
    </div>
  );
}
