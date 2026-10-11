/**
 * The button that shuts every row of a bin's tree.
 *
 * After LTK Manager's `src/modules/workshop/shared/components/CollapseAllButton.tsx`, over this
 * app's button and tooltip.
 */
import { ArrowsInLineVerticalIcon } from "@phosphor-icons/react";

import { Button, Tooltip } from "@/components";
import { m } from "@/i18n";

/** The header's button that shuts every row of the tree, as `Ctrl+←` does. */
export function CollapseAllButton({
  onCollapse,
  disabled,
}: {
  onCollapse: () => void;
  /** Nothing is open. */
  disabled: boolean;
}) {
  return (
    <Tooltip content={m.bin_collapse_all_hint()}>
      <Button
        size="xs"
        compact
        variant="ghost"
        aria-label={m.bin_collapse_all_action()}
        data-ui="BinTree:collapse-all"
        left={<ArrowsInLineVerticalIcon weight="bold" />}
        disabled={disabled}
        onClick={onCollapse}
      />
    </Tooltip>
  );
}
