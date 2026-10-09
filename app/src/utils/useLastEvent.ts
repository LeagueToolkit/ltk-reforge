import type { EventCallback, UnlistenFn } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";

/** An event of the backend's, as the bindings make it. */
interface Listened<T> {
  listen: (handler: EventCallback<T>) => Promise<UnlistenFn>;
}

/** The payload of the last event of the kind heard while the component is mounted, else null. */
export function useLastEvent<T>(event: Listened<T>): T | null {
  const [last, setLast] = useState<T | null>(null);
  useEffect(() => {
    const unlisten = event.listen(({ payload }) => setLast(payload));
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [event]);
  return last;
}
