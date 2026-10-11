import { XIcon } from "@phosphor-icons/react";

import { Button, FailureNote, SectionCard, Tooltip } from "@/components";
import { m } from "@/i18n";
import { twMerge } from "@/utils";

import { addFiles, useAddFolder } from "../api/batch";
import { useBatchStore } from "../state/batchStore";

/** The file name of a path. */
export function fileNameOf(path: string): string {
  const cut = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return cut >= 0 ? path.slice(cut + 1) : path;
}

/**
 * The mods of the batch: a folder's archives, files picked, or files dropped on the window,
 * each once. The list does not change while the batch runs.
 */
export function ModsCard({ busy, over }: { busy: boolean; over: boolean }) {
  const archives = useBatchStore((state) => state.archives);
  const addArchives = useBatchStore((state) => state.addArchives);
  const removeArchive = useBatchStore((state) => state.removeArchive);
  const clear = useBatchStore((state) => state.clear);
  const addFolder = useAddFolder();

  const actions = (
    <div className="flex flex-wrap gap-2">
      <Button
        size="sm"
        variant="outline"
        disabled={busy}
        loading={addFolder.isPending}
        onClick={() => addFolder.mutate()}
      >
        {m.batch_mods_choose_folder_action()}
      </Button>
      <Button
        size="sm"
        variant="outline"
        disabled={busy}
        onClick={() => void addFiles(addArchives)}
      >
        {m.batch_mods_add_files_action()}
      </Button>
      <Button size="sm" variant="ghost" disabled={busy || archives.length === 0} onClick={clear}>
        {m.batch_mods_clear_action()}
      </Button>
    </div>
  );

  return (
    <SectionCard title={m.batch_mods_title()} action={actions}>
      <div
        className={twMerge(
          "flex flex-col gap-2 rounded-md",
          over && !busy && "outline-2 outline-offset-8 outline-accent-500 outline-dashed",
        )}
        data-ui="ModsCard:list"
      >
        {addFolder.isError && <FailureNote failure={addFolder.error} />}
        {archives.length === 0 ? (
          <p className="text-sm text-surface-400 select-none">
            {over && !busy ? m.batch_mods_drop_label() : m.batch_mods_none()}
          </p>
        ) : (
          <>
            <p className="text-xs text-surface-400 select-none">
              {m.batch_mods_count_label({ count: archives.length })}
            </p>
            <ul className="flex max-h-64 flex-col overflow-y-auto">
              {archives.map((archive) => {
                const name = fileNameOf(archive);
                return (
                  <li
                    key={archive}
                    className="flex items-center justify-between gap-2 rounded-md px-2 py-1 hover:bg-surface-veil"
                  >
                    <Tooltip content={archive}>
                      <span className="min-w-0 truncate text-sm text-surface-100">{name}</span>
                    </Tooltip>
                    <Button
                      size="xs"
                      variant="ghost"
                      compact
                      disabled={busy}
                      aria-label={m.batch_mods_remove_label({ name })}
                      left={<XIcon weight="bold" />}
                      onClick={() => removeArchive(archive)}
                    />
                  </li>
                );
              })}
            </ul>
          </>
        )}
      </div>
    </SectionCard>
  );
}
