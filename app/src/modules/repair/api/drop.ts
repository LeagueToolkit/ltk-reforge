import { getCurrentWebview } from "@tauri-apps/api/webview";
import { useEffect, useRef, useState } from "react";

/** The extensions a mod archive is picked or dropped by. */
export const ARCHIVE_EXTENSIONS = ["fantome", "zip"] as const;

/** The first path dropped that is a mod archive by its extension, or null when none is. */
export function droppedArchive(paths: readonly string[]): string | null {
  const archive = paths.find((path) => {
    const lower = path.toLowerCase();
    return ARCHIVE_EXTENSIONS.some((extension) => lower.endsWith(`.${extension}`));
  });
  return archive ?? null;
}

/**
 * Files dragged over the window, from the webview's drag-drop events, which the window raises
 * wherever the files are over it. True while a mod archive is held over the window; `onDrop`
 * gets the archive dropped. The listener is kept for the component's life, so a change of
 * `onDrop` does not lose a drag under way.
 */
export function useArchiveDrop(onDrop: (archive: string) => void): boolean {
  const [over, setOver] = useState(false);
  const handler = useRef(onDrop);
  handler.current = onDrop;
  useEffect(() => {
    const stop = getCurrentWebview().onDragDropEvent(({ payload }) => {
      if (payload.type === "enter") {
        setOver(droppedArchive(payload.paths) !== null);
      } else if (payload.type === "leave") {
        setOver(false);
      } else if (payload.type === "drop") {
        setOver(false);
        const archive = droppedArchive(payload.paths);
        if (archive) {
          handler.current(archive);
        }
      }
    });
    return () => {
      void stop.then((unlisten) => unlisten());
    };
  }, []);
  return over;
}
