import { open, save } from "@tauri-apps/plugin-dialog";

/** A folder the user picks, or null when they cancel. */
export async function pickFolder(): Promise<string | null> {
  const picked = await open({ directory: true, multiple: false });
  return typeof picked === "string" ? picked : null;
}

/** A path the user picks to save a file at, or null when they cancel. */
export async function pickSavePath(extensions: string[], name: string): Promise<string | null> {
  return await save({ filters: [{ name, extensions }] });
}

/** A file the user picks, or null when they cancel. */
export async function pickFile(extensions: string[], name: string): Promise<string | null> {
  const picked = await open({ multiple: false, filters: [{ name, extensions }] });
  return typeof picked === "string" ? picked : null;
}
