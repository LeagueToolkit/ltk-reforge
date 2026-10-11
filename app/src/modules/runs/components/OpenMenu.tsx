import { CaretDownIcon, FolderOpenIcon } from "@phosphor-icons/react";

import { Button, Menu } from "@/components";
import { m } from "@/i18n";
import { pickFile } from "@/modules/inputs";

import { type Opened, PACKAGE_EXTENSION, PROJECT_EXTENSION } from "../api/drop";

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
    <Menu
      trigger={
        <Button
          size="sm"
          variant="outline"
          left={<FolderOpenIcon weight="bold" />}
          right={<CaretDownIcon weight="bold" />}
          loading={pending}
          data-ui="OpenMenu"
        >
          {m.runs_open_action()}
        </Button>
      }
      items={[
        { label: m.runs_open_project_action(), onSelect: () => void pick("project") },
        { label: m.runs_open_package_action(), onSelect: () => void pick("package") },
      ]}
    />
  );
}
