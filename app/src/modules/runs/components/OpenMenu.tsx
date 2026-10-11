import { Menu } from "@base-ui/react/menu";
import { CaretDownIcon, FolderOpenIcon } from "@phosphor-icons/react";

import { Button } from "@/components";
import { m } from "@/i18n";
import { pickFile } from "@/modules/inputs";

import { type Opened, PACKAGE_EXTENSION, PROJECT_EXTENSION } from "../api/drop";
import { itemClass } from "./RemoveMenu";

interface OpenMenuProps {
  /** Called with the file picked and what it opens as. */
  onOpen: (opened: Opened) => void;
  /** Whether a file is being opened. */
  pending: boolean;
}

/** What the Runs page opens as a kept run (#ADR-0096): a project file, or a package. */
export function OpenMenu({ onOpen, pending }: OpenMenuProps) {
  const pick = async (kind: Opened["kind"]) => {
    const path =
      kind === "project"
        ? await pickFile([PROJECT_EXTENSION], m.runs_open_project_filter())
        : await pickFile([PACKAGE_EXTENSION], m.runs_open_package_filter());
    if (path) {
      onOpen({ kind, path });
    }
  };

  return (
    <Menu.Root>
      <Menu.Trigger
        render={
          <Button
            size="sm"
            variant="outline"
            left={<FolderOpenIcon weight="bold" />}
            right={<CaretDownIcon weight="bold" />}
            loading={pending}
          />
        }
        data-ui="OpenMenu"
      >
        {m.runs_open_action()}
      </Menu.Trigger>
      <Menu.Portal>
        <Menu.Positioner side="bottom" align="end" sideOffset={6} className="z-50">
          <Menu.Popup className="flex min-w-48 flex-col rounded-lg border border-surface-700 bg-surface-800 p-1 shadow-lg">
            <Menu.Item className={itemClass} onClick={() => void pick("project")}>
              {m.runs_open_project_action()}
            </Menu.Item>
            <Menu.Item className={itemClass} onClick={() => void pick("package")}>
              {m.runs_open_package_action()}
            </Menu.Item>
          </Menu.Popup>
        </Menu.Positioner>
      </Menu.Portal>
    </Menu.Root>
  );
}
