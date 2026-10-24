import { CopyIcon } from "@phosphor-icons/react";

import { Button, Dialog } from "@/components";
import { m } from "@/i18n";
import type { BatchRow } from "@/lib/bindings.gen";
import { copyText } from "@/utils";

import { rowText } from "../summary";

interface MovedDialogProps {
  open: boolean;
  onClose: () => void;
  /** The rows a base unit of which was placed at a premium slot. */
  rows: readonly BatchRow[];
}

/**
 * Told once a batch ends with mods moved from the base to a premium slot (#ADR-0013): the
 * mods still work, at that skin, and why. The lines of those mods alone are copied, to be
 * reported.
 */
export function MovedDialog({ open, onClose, rows }: MovedDialogProps) {
  const lines = rows.map(rowText).join("\n");
  return (
    <Dialog
      open={open}
      onClose={onClose}
      title={m.batch_moved_title({ count: rows.length })}
      description={m.batch_moved_description()}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            {m.batch_moved_close_action()}
          </Button>
          <Button
            variant="filled"
            left={<CopyIcon weight="bold" />}
            onClick={() => copyText(lines)}
          >
            {m.batch_moved_copy_action()}
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-3" data-ui="MovedDialog">
        <p className="text-sm text-surface-300">{m.batch_moved_why()}</p>
        <pre className="max-h-64 overflow-auto rounded-md border border-surface-700 bg-surface-950/60 p-3 font-mono text-xs break-all whitespace-pre-wrap text-surface-100">
          {lines}
        </pre>
        <p className="text-sm text-surface-300">{m.batch_moved_report()}</p>
      </div>
    </Dialog>
  );
}
