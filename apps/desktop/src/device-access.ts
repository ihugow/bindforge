import { invoke } from "@tauri-apps/api/core";
export type AccessStatus = {
  fingerprint: string;
  device_name: string; model: string;
  readable: boolean; output_writable: boolean; managed: boolean; revoked: boolean; helper_available: boolean;
  scope: string; message: string;
};
export type AccessResult = { status: AccessStatus; reconnect_required: boolean; message: string };
export type AccessAction = "grant" | "revoke";
export const deviceAccessApi = {
  status: (deviceId: string) => invoke<AccessStatus>("get_device_access", { deviceId }),
  control: (deviceId: string, action: AccessAction, fingerprint: string) => invoke<AccessResult>("configure_device_access", { deviceId, action, fingerprint }),
};
