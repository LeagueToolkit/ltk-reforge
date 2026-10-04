import { FailureNote, SectionCard, SegmentedControl, Spinner } from "@/components";
import { m } from "@/i18n";
import type { Theme } from "@/lib/bindings.gen";

import { useSaveSettings, useSettings } from "../api/settings";

const THEMES: Theme[] = ["system", "dark", "light"];

/** The settings, a section per domain. */
export function SettingsPage() {
  const settings = useSettings();
  const save = useSaveSettings();

  if (settings.isPending) {
    return <Spinner />;
  }
  if (settings.isError) {
    return <FailureNote failure={settings.error} />;
  }

  const current = settings.data;
  const options = THEMES.map((theme) => ({ value: theme, label: m[`theme.${theme}.label`]() }));

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
        {save.isError && <FailureNote failure={save.error} />}
      </SectionCard>
    </div>
  );
}
