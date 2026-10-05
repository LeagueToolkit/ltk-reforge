import { Button, Dialog } from "@/components";
import { m } from "@/i18n";

interface ConfirmRemoveProps {
  open: boolean;
  onClose: () => void;
  /** What is removed, as a question. */
  title: string;
  /** What is freed, and what having it again costs. */
  description: string;
  onConfirm: () => void;
}

/** A removal from disk, asked about before it is made. */
export function ConfirmRemove({
  open,
  onClose,
  title,
  description,
  onConfirm,
}: ConfirmRemoveProps) {
  return (
    <Dialog
      open={open}
      onClose={onClose}
      title={title}
      description={description}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            {m.inputs_remove_cancel_action()}
          </Button>
          <Button
            variant="danger"
            onClick={() => {
              onConfirm();
              onClose();
            }}
          >
            {m.inputs_remove_confirm_action()}
          </Button>
        </>
      }
    />
  );
}
