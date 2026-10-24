import { CaretDownIcon } from "@phosphor-icons/react";
// The inputs module's picker is behind a barrel that reads the settings: a cycle.
import { open } from "@tauri-apps/plugin-dialog";
import type { ReactNode } from "react";

import { Button, FailureNote, Menu, SectionCard, SegmentedControl, Spinner } from "@/components";
import { m } from "@/i18n";
import type { Theme } from "@/lib/bindings.gen";

import { useSaveSettings, useSettings } from "../api/settings";
import { DEFAULT_PACKAGE_FORMAT, formatLabel, PACKAGE_FORMATS } from "../format";
import { AUTOMATIC_JOBS, jobsChoices, jobsLabel, useBatchJobs } from "../jobs";

const THEMES: Theme[] = ["system", "dark", "light"];

interface SettingsPageProps {
  /**
   * The rows of the storage group: what the app keeps on disk. Modules that read the settings
   * give them, through the route, so this module imports none of them.
   */
  storage?: ReactNode;
}

/** The settings, a section per domain. */
export function SettingsPage({ storage }: SettingsPageProps) {
  const settings = useSettings();
  const save = useSaveSettings();
  const jobs = useBatchJobs();

  if (settings.isPending) {
    return <Spinner />;
  }
  if (settings.isError) {
    return <FailureNote failure={settings.error} />;
  }

  const current = settings.data;
  const options = THEMES.map((theme) => ({ value: theme, label: m[`theme.${theme}.label`]() }));
  const formats = PACKAGE_FORMATS.map((format) => ({
    value: format,
    label: formatLabel(format),
  }));
  const chosenOut = current["output.folder"];
  const chosenJobs = current["batch.jobs"] ?? AUTOMATIC_JOBS;
  const jobsItems = jobsChoices(jobs.data ?? { automatic: 1, max: 1 }).map((choice) => ({
    label: jobsLabel(choice, jobs.data),
    disabled: choice === chosenJobs,
    onSelect: () => save.mutate({ ...current, "batch.jobs": choice }),
  }));

  const chooseOut = async () => {
    const picked = await open({ directory: true, multiple: false });
    if (typeof picked === "string") {
      save.mutate({ ...current, "output.folder": picked });
    }
  };
  const resetOut = () => save.mutate({ ...current, "output.folder": null });

  return (
    <div className="flex max-w-2xl flex-col gap-6" data-ui="SettingsPage">
      <h1 className="font-display text-xl font-semibold text-surface-50">
        {m.settings_page_title()}
      </h1>
      <SectionCard title={m.settings_appearance_title()}>
        <div className="flex items-center justify-between gap-4">
          <div>
            <p className="text-sm text-surface-100">{m["setting.appearance.theme.label"]()}</p>
            <p className="text-xs text-surface-400">
              {m["setting.appearance.theme.description"]()}
            </p>
          </div>
          <SegmentedControl
            options={options}
            value={current["appearance.theme"] ?? "system"}
            onChange={(theme) => save.mutate({ ...current, "appearance.theme": theme })}
            aria-label={m["setting.appearance.theme.label"]()}
          />
        </div>
      </SectionCard>
      <SectionCard title={m.settings_output_title()}>
        <div className="flex items-center justify-between gap-4" data-ui="SettingsPage:format">
          <div>
            <p className="text-sm text-surface-100">{m["setting.output.format.label"]()}</p>
            <p className="text-xs text-surface-400">{m["setting.output.format.description"]()}</p>
          </div>
          <SegmentedControl
            options={formats}
            value={current["output.format"] ?? DEFAULT_PACKAGE_FORMAT}
            onChange={(format) => save.mutate({ ...current, "output.format": format })}
            aria-label={m["setting.output.format.label"]()}
          />
        </div>
        <div className="flex items-start justify-between gap-4" data-ui="SettingsPage:output">
          <div className="min-w-0 flex-1">
            <p className="text-sm text-surface-100">{m["setting.output.folder.label"]()}</p>
            {chosenOut ? (
              <p className="font-mono text-xs break-all text-surface-300">{chosenOut}</p>
            ) : (
              <p className="text-xs text-surface-400">{m["setting.output.folder.description"]()}</p>
            )}
          </div>
          <div className="flex shrink-0 gap-2">
            {chosenOut && (
              <Button size="sm" variant="ghost" onClick={resetOut}>
                {m.settings_output_reset_action()}
              </Button>
            )}
            <Button size="sm" variant="outline" onClick={() => void chooseOut()}>
              {m.settings_output_choose_action()}
            </Button>
          </div>
        </div>
      </SectionCard>
      <SectionCard title={m.settings_batch_title()}>
        <div className="flex items-center justify-between gap-4" data-ui="SettingsPage:jobs">
          <div>
            <p className="text-sm text-surface-100">{m["setting.batch.jobs.label"]()}</p>
            <p className="text-xs text-surface-400">{m["setting.batch.jobs.description"]()}</p>
          </div>
          <Menu
            trigger={
              <Button
                size="sm"
                variant="outline"
                right={<CaretDownIcon weight="bold" />}
                disabled={!jobs.data}
                aria-label={m["setting.batch.jobs.label"]()}
              >
                {jobsLabel(chosenJobs, jobs.data)}
              </Button>
            }
            items={jobsItems}
          />
        </div>
      </SectionCard>
      {storage && <SectionCard title={m.settings_storage_title()}>{storage}</SectionCard>}
      {save.isError && <FailureNote failure={save.error} />}
    </div>
  );
}
