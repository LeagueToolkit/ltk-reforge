import { open } from "@tauri-apps/plugin-dialog";

/** A folder the user picks, or null when they cancel. */
export async function pickFolder(): Promise<string | null> {
  const picked = await open({ directory: true, multiple: false });
  return typeof picked === "string" ? picked : null;
}

/** A file the user picks, or null when they cancel. */
export async function pickFile(extensions: string[], name: string): Promise<string | null> {
  const picked = await open({ multiple: false, filters: [{ name, extensions }] });
  return typeof picked === "string" ? picked : null;
}
