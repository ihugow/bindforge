import { batteryLabel, type DetectedDevice } from "./devices";

export function DeviceBattery({ device }: { device?: DetectedDevice }) {
  const available = device?.battery && Number.isInteger(device.battery.percentage) && device.battery.percentage >= 0 && device.battery.percentage <= 100;
  return <span className={`device-battery${available && device.battery!.percentage <= 20 ? " battery-low" : ""}`}>
    {available && <svg viewBox="0 0 26 14" width="26" height="14" aria-hidden="true">
      <rect x="1" y="1" width="21" height="12" rx="2" fill="none" stroke="currentColor" strokeWidth="1.5" />
      <path d="M24 5v4" stroke="currentColor" strokeWidth="2" />
      <rect x="3.5" y="3.5" width={16 * device.battery!.percentage / 100} height="7" rx="1" fill="currentColor" />
    </svg>}
    {batteryLabel(device)}
  </span>;
}
