import { Button, SectionCard } from "@/components";
import { m } from "@/i18n";
import { pickFile, pickFolder } from "@/modules/inputs";
import { useSaveSettings, useSettings } from "@/modules/settings";

import { folderOf } from "../api/run";
import { useRepairStore } from "../state/repairStore";

/** The folder a package is written in: the chosen one, else the mod's own. */
export function useOutputFolder(): string | null {
  const settings = useSettings();
  const archive = useRepairStore((state) => state.archive);
  const chosen = settings.data?.["output.folder"];
  if (chosen) {
    return chosen;
  }
  return archive ? folderOf(archive) : null;
}

/** The mod to repair, as picked and as the last run named it, and where it is written. */
export function ModCard() {
  const archive = useRepairStore((state) => state.archive);
  const setArchive = useRepairStore((state) => state.setArchive);
  const meta = useRepairStore((state) => state.last?.meta);
  const thumbnail = useRepairStore(
    (state) => (state.live ? state.live.package : state.last?.package)?.thumbnail,
  );
  const settings = useSettings();
  const save = useSaveSettings();
  const out = useOutputFolder();
  const chosenOut = settings.data?.["output.folder"];

  const chooseMod = async () => {
    const picked = await pickFile(["fantome", "zip"], m.repair_archive_filter_label());
    if (picked) {
      setArchive(picked);
    }
  };
  const chooseOut = async () => {
    const picked = await pickFolder();
    if (picked && settings.data) {
      save.mutate({ ...settings.data, "output.folder": picked });
    }
  };
  const resetOut = () => {
    if (settings.data) {
      save.mutate({ ...settings.data, "output.folder": null });
    }
  };

  return (
    <SectionCard title={m.repair_mod_title()}>
      <div className="flex items-start justify-between gap-4" data-ui="ModCard:mod">
        {thumbnail && (
          <img
            src={thumbnail}
            alt={m.repair_mod_thumbnail_alt()}
            className="h-16 w-16 shrink-0 rounded-md border border-surface-700 object-cover"
          />
        )}
        <div className="min-w-0 flex-1">
          {meta?.name && <p className="text-sm text-surface-50">{meta.name}</p>}
          {meta?.author && (
            <p className="text-xs text-surface-300">
              {m.repair_mod_by_label({ author: meta.author })}
            </p>
          )}
          {archive && <p className="font-mono text-xs break-all text-surface-300">{archive}</p>}
          {!archive && (
            <p className="text-sm text-surface-400 select-none">{m.repair_mod_none()}</p>
          )}
        </div>
        <Button size="sm" variant="outline" onClick={() => void chooseMod()}>
          {m.repair_mod_choose_action()}
        </Button>
      </div>
      <div className="flex items-start justify-between gap-4" data-ui="ModCard:out">
        <div className="min-w-0 flex-1">
          <p className="text-xs text-surface-400 select-none">{m.repair_out_title()}</p>
          {out && <p className="font-mono text-xs break-all text-surface-300">{out}</p>}
          {!chosenOut && (
            <p className="text-xs text-surface-400 select-none">
              {m.repair_out_mod_folder_label()}
            </p>
          )}
        </div>
        <div className="flex shrink-0 gap-2">
          {chosenOut && (
            <Button size="sm" variant="ghost" onClick={resetOut}>
              {m.repair_out_reset_action()}
            </Button>
          )}
          <Button size="sm" variant="outline" onClick={() => void chooseOut()}>
            {m.repair_out_choose_action()}
          </Button>
        </div>
      </div>
    </SectionCard>
  );
}
