import { Button } from "@/components";
import { m } from "@/i18n";
import type { InstallProgress } from "@/lib/bindings.gen";

interface ProgressRowProps {
  /** How much is done, from 0 to 1. Null when the whole is not known: no bar is drawn. */
  fraction: number | null;
  /** How far it has got, in words. */
  label: string;
  /** Stops the work. None when it cannot be stopped. */
  onStop?: () => void;
}

/** Work under way: a bar of what is done, a label, and a button that stops it. */
export function ProgressRow({ fraction, label, onStop }: ProgressRowProps) {
  return (
    <div className="flex items-center gap-3" data-ui="ProgressRow">
      {fraction != null && (
        <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-surface-700">
          <div
            className="h-full bg-accent-500 transition-[width]"
            style={{ width: `${Math.min(fraction, 1) * 100}%` }}
          />
        </div>
      )}
      <span className="font-mono text-xs text-surface-300">{label}</span>
      {onStop && (
        <Button size="xs" variant="ghost" onClick={onStop}>
          {m.inputs_stop_action()}
        </Button>
      )}
    </div>
  );
}

/** A download of a release's file, its check, or its unpacking after, as a progress row. */
export function StepProgress({
  progress,
  onStop,
}: {
  progress: InstallProgress;
  onStop: () => void;
}) {
  const sizes = { done: megabytes(progress.done), total: megabytes(progress.total) };
  const labels = {
    check: m.inputs_check_progress_label,
    download: m.inputs_download_progress_label,
    unpack: m.inputs_unpack_progress_label,
  };
  return (
    <ProgressRow
      fraction={progress.done / Math.max(progress.total, 1)}
      label={labels[progress.step](sizes)}
      onStop={onStop}
    />
  );
}

/** Bytes as whole megabytes. */
export function megabytes(bytes: number): string {
  return (bytes / 1_000_000).toFixed(0);
}

/** Bytes on disk with their unit: tenths of a gigabyte, whole megabytes or whole kilobytes. */
export function diskSize(bytes: number): string {
  if (bytes >= 1_000_000_000) {
    return `${(bytes / 1_000_000_000).toFixed(1)} GB`;
  }
  if (bytes >= 1_000_000) {
    return `${megabytes(bytes)} MB`;
  }
  return `${kilobytes(bytes)} KB`;
}

/** Bytes as whole kilobytes. */
export function kilobytes(bytes: number): string {
  return (bytes / 1_000).toFixed(0);
}
