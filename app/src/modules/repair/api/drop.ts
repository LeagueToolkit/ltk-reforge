import { getCurrentWebview } from "@tauri-apps/api/webview";
import { useEffect, useRef, useState } from "react";

/** The extensions a mod archive is picked or dropped by. */
export const ARCHIVE_EXTENSIONS = ["fantome", "zip"] as const;

/** The paths dropped that are mod archives by their extension, in the order dropped. */
export function droppedArchives(paths: readonly string[]): string[] {
  return paths.filter((path) => {
    const lower = path.toLowerCase();
    return ARCHIVE_EXTENSIONS.some((extension) => lower.endsWith(`.${extension}`));
  });
}

/**
 * Files dragged over the window, from the webview's drag-drop events, which the window raises
 * wherever the files are over it. True while a mod archive is held over the window; `onDrop`
 * gets every archive dropped. The listener is kept for the component's life, so a change of
 * `onDrop` does not lose a drag under way.
 */
export function useArchiveDrop(onDrop: (archives: string[]) => void): boolean {
  const [over, setOver] = useState(false);
  const handler = useRef(onDrop);
  handler.current = onDrop;
  useEffect(() => {
    const stop = getCurrentWebview().onDragDropEvent(({ payload }) => {
      if (payload.type === "enter") {
        setOver(droppedArchives(payload.paths).length > 0);
      } else if (payload.type === "leave") {
        setOver(false);
      } else if (payload.type === "drop") {
        setOver(false);
        const archives = droppedArchives(payload.paths);
        if (archives.length > 0) {
          handler.current(archives);
        }
      }
    });
    return () => {
      void stop.then((unlisten) => unlisten());
    };
  }, []);
  return over;
}
