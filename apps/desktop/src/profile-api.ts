import { invoke } from "@tauri-apps/api/core";
import type { Config } from "./profiles";
export const profileApi = {
  load: () => invoke<Config>("load_profiles"),
  save: (config: Config, expectedRevision: number) => invoke<Config>("save_profiles", { config, expectedRevision }),
};
