/** The header's button that copies the bin's path, in the shape of `CollapseAllButton`. */
import { CopyIcon } from "@phosphor-icons/react";

import { Button, Tooltip } from "@/components";
import { m } from "@/i18n";
import { copyText } from "@/utils";

/** The header's button that copies `path`, the bin's path as the viewer shows it. */
export function CopyPathButton({ path }: { path: string }) {
  return (
    <Tooltip content={m.bin_copy_bin_path_action()}>
      <Button
        size="xs"
        compact
        variant="ghost"
        aria-label={m.bin_copy_bin_path_action()}
        data-ui="BinTree:copy-path"
        left={<CopyIcon weight="bold" />}
        onClick={() => copyText(path)}
      />
    </Tooltip>
  );
}
