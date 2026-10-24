import { CaretDownIcon } from "@phosphor-icons/react";

import { Button, Menu } from "@/components";
import { m } from "@/i18n";
import {
  DEFAULT_PACKAGE_FORMAT,
  formatLabel,
  PACKAGE_FORMATS,
  useSaveSettings,
  useSettings,
} from "@/modules/settings";

/**
 * The form the package is written in, chosen beside the run's button. It is the setting under
 * Output, so a choice here holds for every run (#ADR-0105).
 */
export function PackageFormatMenu() {
  const settings = useSettings();
  const save = useSaveSettings();
  const current = settings.data;
  const format = current?.["output.format"] ?? DEFAULT_PACKAGE_FORMAT;
  const items = PACKAGE_FORMATS.map((option) => ({
    label: formatLabel(option),
    disabled: option === format,
    onSelect: () => {
      if (current) {
        save.mutate({ ...current, "output.format": option });
      }
    },
  }));
  return (
    <Menu
      trigger={
        <Button
          size="sm"
          variant="outline"
          right={<CaretDownIcon weight="bold" />}
          disabled={!current}
          aria-label={m["setting.output.format.label"]()}
          data-ui="PackageFormatMenu"
        >
          {formatLabel(format)}
        </Button>
      }
      items={items}
    />
  );
}
