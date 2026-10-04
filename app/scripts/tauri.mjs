// The Tauri CLI, pointed at the app's crate. The crate is in the workspace's `crates/` and the
// front end in `app/`, so the CLI finds neither by walking from here.
import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const app = path.dirname(path.dirname(fileURLToPath(import.meta.url)));

const result = spawnSync("tauri", process.argv.slice(2), {
  stdio: "inherit",
  shell: true,
  env: {
    ...process.env,
    TAURI_APP_PATH: path.join(app, "..", "crates", "reforge-app"),
    TAURI_FRONTEND_PATH: app,
  },
});

process.exit(result.status ?? 1);
