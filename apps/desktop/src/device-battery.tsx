import { batteryLabel, type DetectedDevice } from "./devices";

export function DeviceBattery({ device }: { device?: DetectedDevice }) {
  const available = device?.battery && Number.isInteger(device.battery.percentage) && device.battery.percentage >= 0 && device.battery.percentage <= 100;
  return <span className={`device-battery${available && device.battery!.percentage <= 20 ? " battery-low" : ""}`}>
    {available && <svg viewBox="0 0 26 14" width="26" height="14" aria-hidden="true">
      <rect x="0" y="0" width="22" height="14" rx="3" fill="currentColor" opacity=".25" />
      <rect x="23" y="4" width="3" height="6" rx="1" fill="currentColor" />
      <rect x="2" y="2" width={18 * device.battery!.percentage / 100} height="10" rx="1.5" fill="currentColor" />
    </svg>}
    {batteryLabel(device)}
  </span>;
}
