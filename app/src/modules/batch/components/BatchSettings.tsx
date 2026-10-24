import { Link } from "@tanstack/react-router";

import { SectionCard, SegmentedControl, Switch } from "@/components";
import { m } from "@/i18n";
import type { RunKind } from "@/lib/bindings.gen";
import {
  DEFAULT_PACKAGE_FORMAT,
  formatLabel,
  PACKAGE_FORMATS,
  useSaveSettings,
  useSettings,
} from "@/modules/settings";

import { useBatchStore } from "../state/batchStore";

/**
 * What every run of the batch is made with: the kind, where the packages go, and the standard
 * settings or the run page's advanced ones.
 */
export function BatchSettings({ busy }: { busy: boolean }) {
  const kind = useBatchStore((state) => state.kind);
  const setKind = useBatchStore((state) => state.setKind);
  const runPageSettings = useBatchStore((state) => state.runPageSettings);
  const setRunPageSettings = useBatchStore((state) => state.setRunPageSettings);
  const settings = useSettings().data;
  const save = useSaveSettings();
  const chosenOut = settings?.["output.folder"];
  const format = settings?.["output.format"] ?? DEFAULT_PACKAGE_FORMAT;
  const formats = PACKAGE_FORMATS.map((option) => ({
    value: option,
    label: formatLabel(option),
  }));

  const kinds: { value: RunKind; label: string }[] = [
    { value: "repair", label: m.batch_kind_repair_label() },
    { value: "port", label: m.batch_kind_port_label() },
  ];

  return (
    <SectionCard title={m.batch_settings_title()}>
      <div className="flex items-center justify-between gap-4">
        <p className="text-sm text-surface-100 select-none">{m.batch_kind_label()}</p>
        <SegmentedControl
          options={kinds}
          value={kind}
          onChange={(next) => !busy && setKind(next)}
          aria-label={m.batch_kind_label()}
        />
      </div>
      <div className="flex items-center justify-between gap-4" data-ui="BatchSettings:format">
        <p className="text-sm text-surface-100 select-none">{m["setting.output.format.label"]()}</p>
        <SegmentedControl
          options={formats}
          value={format}
          onChange={(next) =>
            !busy && settings && save.mutate({ ...settings, "output.format": next })
          }
          aria-label={m["setting.output.format.label"]()}
        />
      </div>
      <Link
        to="/settings"
        className="text-xs break-all text-surface-400 underline-offset-2 hover:text-surface-200 hover:underline"
        data-ui="BatchSettings:out"
      >
        {chosenOut ? m.batch_out_folder_label({ folder: chosenOut }) : m.batch_out_beside_label()}
      </Link>
      <div className="flex items-start justify-between gap-4">
        <div>
          <p className="text-sm text-surface-100 select-none">
            {m.batch_settings_run_page_label()}
          </p>
          <p className="text-xs text-surface-400 select-none">
            {runPageSettings
              ? m.batch_settings_run_page_description()
              : m.batch_settings_standard_description()}
          </p>
        </div>
        <Switch
          checked={runPageSettings}
          disabled={busy}
          onCheckedChange={setRunPageSettings}
          aria-label={m.batch_settings_run_page_label()}
        />
      </div>
    </SectionCard>
  );
}
