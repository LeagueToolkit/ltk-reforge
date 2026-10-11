import {
  ArrowCircleUpIcon,
  CheckCircleIcon,
  FolderOpenIcon,
  MinusCircleIcon,
  TrashIcon,
  XCircleIcon,
} from "@phosphor-icons/react";
import type { ReactNode } from "react";

import { Button, FailureNote, IconButton, Spinner, Tooltip } from "@/components";
import { m } from "@/i18n";
import type { Place } from "@/lib/bindings.gen";
import { twMerge } from "@/utils";

import { useReveal } from "../api/storage";

/** How an input stands: current, behind what is published, absent, being asked about, or broken. */
export type Tone = "ok" | "warn" | "muted" | "busy" | "bad";

const toneClasses: Record<Tone, string> = {
  ok: "bg-success/10 text-success-text",
  warn: "bg-warning/10 text-warning-text",
  muted: "bg-surface-veil text-surface-300",
  busy: "bg-surface-veil text-surface-300",
  bad: "bg-danger/10 text-danger-text",
};

function ToneIcon({ tone }: { tone: Tone }) {
  const className = "h-3.5 w-3.5";
  switch (tone) {
    case "ok":
      return <CheckCircleIcon weight="fill" className={className} />;
    case "warn":
      return <ArrowCircleUpIcon weight="fill" className={className} />;
    case "muted":
      return <MinusCircleIcon weight="bold" className={className} />;
    case "busy":
      return <Spinner size="sm" className={className} />;
    case "bad":
      return <XCircleIcon weight="fill" className={className} />;
  }
}

/** How an input stands, in a word beside its name. */
export function StatusBadge({ tone, children }: { tone: Tone; children: ReactNode }) {
  return (
    <span
      className={twMerge(
        "inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-xs font-medium whitespace-nowrap select-none",
        toneClasses[tone],
      )}
      data-ui="StatusBadge"
      data-tone={tone}
    >
      <ToneIcon tone={tone} />
      {children}
    </span>
  );
}

/** A path read, and a button that shows its folder. */
export function PathLine({ path, place }: { path: string; place: Place }) {
  const reveal = useReveal();
  return (
    <>
      <div className="flex items-center gap-2 rounded-md bg-surface-veil-soft py-1 pr-1 pl-2.5">
        <p className="min-w-0 flex-1 font-mono text-xs break-all text-surface-200">{path}</p>
        <Tooltip content={m.inputs_reveal_action()}>
          <IconButton
            size="xs"
            compact
            variant="ghost"
            aria-label={m.inputs_reveal_action()}
            icon={<FolderOpenIcon weight="bold" />}
            onClick={() => reveal.mutate(place)}
          />
        </Tooltip>
      </div>
      {reveal.isError && <FailureNote failure={reveal.error} className="text-xs" />}
    </>
  );
}

interface InputRowProps {
  title: string;
  /** What kind of thing it is on disk, when the name does not say: a database, a repository. */
  kind?: string;
  /** How it stands. */
  badge?: ReactNode;
  description: string;
  /** The path read, or null. */
  path?: string | null;
  /** Which folder the path's button shows. */
  place?: Place;
  /** What the row says when no path was read. */
  none?: string;
  /** Short facts under the path: where it came from, a build, a size. */
  facts?: ReactNode;
  actions?: ReactNode;
  /** What can be done about it, under the facts: an offer, work under way, a failure. */
  children?: ReactNode;
}

/** One thing the inputs page lists: its name and state, where it is, and what can be done. */
export function InputRow({
  title,
  kind,
  badge,
  description,
  path,
  place,
  none,
  facts,
  actions,
  children,
}: InputRowProps) {
  return (
    <div className="flex flex-col gap-2 py-4 first:pt-0 last:pb-0" data-ui="InputRow">
      <div className="flex items-start justify-between gap-4">
        <div className="min-w-0 flex-1">
          <p className="flex flex-wrap items-center gap-2 text-sm font-medium text-surface-100 select-none">
            {title}
            {kind && <span className="text-xs font-normal text-surface-400">{kind}</span>}
            {badge}
          </p>
          <p className="mt-0.5 text-xs text-surface-400 select-none">{description}</p>
        </div>
        {actions && <div className="flex shrink-0 flex-wrap justify-end gap-2">{actions}</div>}
      </div>
      {path && place && <PathLine path={path} place={place} />}
      {!path && none && <p className="text-xs text-surface-300 select-none">{none}</p>}
      {facts && (
        <div className="flex flex-wrap gap-x-4 gap-y-0.5 text-xs text-surface-400 select-none">
          {facts}
        </div>
      )}
      {children}
    </div>
  );
}

interface OfferProps {
  /** What the action would do, with its cost. */
  text: string;
  action?: ReactNode;
  /** Work under way or a failure, under the text. */
  children?: ReactNode;
}

/** Something the user can have done for an input: what it does and costs, and its button. */
export function Offer({ text, action, children }: OfferProps) {
  return (
    <div
      className="flex flex-col gap-2 rounded-lg border border-surface-veil-strong bg-surface-veil-soft px-3 py-2.5"
      data-ui="Offer"
    >
      <div className="flex items-center justify-between gap-3">
        <p className="min-w-0 text-xs text-surface-200 select-none">{text}</p>
        {action && <div className="shrink-0">{action}</div>}
      </div>
      {children}
    </div>
  );
}

interface RemoveButtonProps {
  label: string;
  onClick: () => void;
  disabled?: boolean;
}

/** A button that removes something from disk. */
export function RemoveButton({ label, onClick, disabled }: RemoveButtonProps) {
  return (
    <Button
      size="sm"
      variant="ghost"
      className="text-danger-text"
      left={<TrashIcon weight="bold" />}
      disabled={disabled}
      onClick={onClick}
    >
      {label}
    </Button>
  );
}
