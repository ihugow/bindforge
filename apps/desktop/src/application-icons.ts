import { invoke } from "@tauri-apps/api/core";

export type ApplicationIcon = { app_id: string; name: string; icon: string };
export type FindApplicationIcons = (appIds: string[]) => Promise<ApplicationIcon[]>;
export const findApplicationIcons: FindApplicationIcons = appIds => invoke("find_application_icons", { appIds });
