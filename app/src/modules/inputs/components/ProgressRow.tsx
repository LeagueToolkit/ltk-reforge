import { Button } from "@/components";
import { m } from "@/i18n";

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

/** Bytes as whole megabytes. */
export function megabytes(bytes: number): string {
  return (bytes / 1_000_000).toFixed(0);
}

/** Bytes as whole kilobytes. */
export function kilobytes(bytes: number): string {
  return (bytes / 1_000).toFixed(0);
}
