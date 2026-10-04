import { Dialog as BaseDialog } from "@base-ui/react/dialog";
import type { ReactNode } from "react";

export interface DialogProps {
  open: boolean;
  onClose: () => void;
  title: string;
  description?: string;
  children?: ReactNode;
  /** The buttons along the bottom. */
  footer?: ReactNode;
}

/** A dialog over the page: a title, a body, and its buttons. */
export function Dialog({ open, onClose, title, description, children, footer }: DialogProps) {
  return (
    <BaseDialog.Root open={open} onOpenChange={(next) => !next && onClose()}>
      <BaseDialog.Portal>
        <BaseDialog.Backdrop className="fixed inset-0 z-modal bg-scrim" />
        <BaseDialog.Popup
          className="fixed top-1/2 left-1/2 z-modal flex w-[32rem] max-w-[90vw] -translate-x-1/2 -translate-y-1/2 flex-col gap-4 rounded-xl border border-surface-700 bg-surface-900 p-5 shadow-xl"
          data-ui="Dialog"
        >
          <div>
            <BaseDialog.Title className="font-display text-lg font-semibold text-surface-50">
              {title}
            </BaseDialog.Title>
            {description && (
              <BaseDialog.Description className="mt-1 text-sm text-surface-300">
                {description}
              </BaseDialog.Description>
            )}
          </div>
          {children}
          {footer && <div className="flex justify-end gap-2">{footer}</div>}
        </BaseDialog.Popup>
      </BaseDialog.Portal>
    </BaseDialog.Root>
  );
}
