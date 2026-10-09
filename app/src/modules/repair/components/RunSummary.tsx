import { CheckCircleIcon, WarningIcon, XCircleIcon } from "@phosphor-icons/react";
import { useMutation } from "@tanstack/react-query";
import type { ReactNode } from "react";

import { Button, FailureNote, Tooltip } from "@/components";
import { m } from "@/i18n";
import { commands, type RunView } from "@/lib/bindings.gen";
import { countBySeverity } from "@/modules/findings";
import { mutationFn, twMerge } from "@/utils";

interface Banner {
  tone: "success" | "warning" | "danger" | "neutral";
  title: string;
  description?: string;
  detail?: string;
  /** The written package's path, shown when the detail is hovered. */
  path?: string;
}

/** The file name of a path. */
function fileNameOf(path: string): string {
  const cut = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return cut >= 0 ? path.slice(cut + 1) : path;
}

function bannerOf(view: RunView): Banner {
  const counts = countBySeverity(view.findings);
  if (view.fault) {
    return { tone: "danger", title: m.repair_status_fault_title(), detail: view.fault };
  }
  if (view.halted != null) {
    const halting = view.findings.find((finding) => finding.id === view.halted);
    return {
      tone: "danger",
      title: m.repair_status_halted_title({ message: halting?.message ?? "" }),
    };
  }
  if (view.written) {
    return {
      tone: "success",
      title: m.repair_status_written_title(),
      detail: fileNameOf(view.written),
      path: view.written,
    };
  }
  if (view.refused) {
    return {
      tone: "warning",
      title: m.repair_status_refused_title({ count: counts.error + counts.fatal }),
      description: m.repair_status_refused_description(),
    };
  }
  const last = [...view.stages].reverse().find((stage) => stage.state === "ran");
  const stage = last ? m[`stage.${last.stage}.label`]() : "";
  return { tone: "neutral", title: m.repair_status_stopped_title({ stage }) };
}

const toneClass: Record<Banner["tone"], string> = {
  success: "border-success/40 bg-success/10",
  warning: "border-warning/40 bg-warning/10",
  danger: "border-danger/40 bg-danger/10",
  neutral: "border-surface-700 bg-surface-900",
};

const toneIcon: Record<Banner["tone"], ReactNode> = {
  success: <CheckCircleIcon weight="bold" className="h-5 w-5 text-success-text" />,
  warning: <WarningIcon weight="bold" className="h-5 w-5 text-warning-text" />,
  danger: <XCircleIcon weight="bold" className="h-5 w-5 text-danger-text" />,
  neutral: null,
};

export interface RunSummaryProps {
  view: RunView;
  /** Writes the package with nothing blocking, when the run's errors refused the write. */
  onWriteAnyway?: () => void;
  writing?: boolean;
}

/**
 * How a run ended: written, refused, halted or stopped, and its findings counted. A written
 * package is named by its file, its path on hover, with a button that shows it in its folder
 * (`reveal_place`, which names the place and never a path).
 */
export function RunSummary({ view, onWriteAnyway, writing }: RunSummaryProps) {
  const banner = bannerOf(view);
  const counts = countBySeverity(view.findings);
  const reveal = useMutation({ mutationFn: mutationFn(commands.revealPlace) });
  const detail = banner.detail ? (
    <p className="font-mono text-xs break-all text-surface-300">{banner.detail}</p>
  ) : null;
  return (
    <div
      className={twMerge("flex items-start gap-3 rounded-xl border p-4", toneClass[banner.tone])}
      data-ui="RunSummary"
    >
      {toneIcon[banner.tone] && <span className="mt-0.5 shrink-0">{toneIcon[banner.tone]}</span>}
      <div className="min-w-0 flex-1">
        <p className="text-sm font-medium text-surface-50">{banner.title}</p>
        {banner.description && (
          <p className="text-xs text-surface-300 select-none">{banner.description}</p>
        )}
        {banner.path && detail ? <Tooltip content={banner.path}>{detail}</Tooltip> : detail}
        {reveal.isError && <FailureNote failure={reveal.error} className="mt-1" />}
        <p className="mt-1 text-xs text-surface-400 select-none">
          {m.repair_summary_counts_label({
            errors: counts.error + counts.fatal,
            warnings: counts.warning,
            notes: counts.note,
          })}{" "}
          {m.repair_summary_time_label({ seconds: (view.millis / 1000).toFixed(1) })}
        </p>
      </div>
      {view.written && (
        <Button
          size="sm"
          variant="outline"
          loading={reveal.isPending}
          onClick={() => reveal.mutate("written")}
        >
          {m.repair_written_show_action()}
        </Button>
      )}
      {view.refused && onWriteAnyway && (
        <Button size="sm" variant="filled" loading={writing} onClick={onWriteAnyway}>
          {m.repair_write_anyway_action()}
        </Button>
      )}
    </div>
  );
}
