import { getCurrentWebview } from "@tauri-apps/api/webview";
import { useEffect, useRef, useState } from "react";

/** The extension of a project file, as the store's `PROJECT_EXTENSION` names it. */
export const PROJECT_EXTENSION = "reforge-project";

/** The extension of a package, whose embedded evidence opens as a run. */
export const PACKAGE_EXTENSION = "modpkg";

/** A file the Runs page opens as a kept run: a project imported, or a package's evidence. */
export interface Opened {
  kind: "project" | "package";
  path: string;
}

/** What a path opens as by its extension, in any case, or null for neither. */
function openedAs(path: string): Opened | null {
  const lower = path.toLowerCase();
  if (lower.endsWith(`.${PROJECT_EXTENSION}`)) {
    return { kind: "project", path };
  }
  if (lower.endsWith(`.${PACKAGE_EXTENSION}`)) {
    return { kind: "package", path };
  }
  return null;
}

/** The first path dropped that is a project file or a package, or null when none is. */
export function droppedOpen(paths: readonly string[]): Opened | null {
  for (const path of paths) {
    const opened = openedAs(path);
    if (opened) {
      return opened;
    }
  }
  return null;
}

/**
 * Files dragged over the window, from the webview's drag-drop events, as the run page takes a
 * mod archive. True while a project file or a package is held over the window; `onDrop` gets
 * the one dropped. The listener is kept for the component's life, so a change of `onDrop` does
 * not lose a drag under way.
 */
export function useOpenDrop(onDrop: (opened: Opened) => void): boolean {
  const [over, setOver] = useState(false);
  const handler = useRef(onDrop);
  handler.current = onDrop;
  useEffect(() => {
    const stop = getCurrentWebview().onDragDropEvent(({ payload }) => {
      if (payload.type === "enter") {
        setOver(droppedOpen(payload.paths) !== null);
      } else if (payload.type === "leave") {
        setOver(false);
      } else if (payload.type === "drop") {
        setOver(false);
        const opened = droppedOpen(payload.paths);
        if (opened) {
          handler.current(opened);
        }
      }
    });
    return () => {
      void stop.then((unlisten) => unlisten());
    };
  }, []);
  return over;
}
