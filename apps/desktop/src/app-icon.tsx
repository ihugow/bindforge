type IconName = "devices" | "profiles" | "registry" | "settings" | "gear" | "spark" | "edit" | "back" | "check" | "chevron-down" | "chevron-up" | "close" | "plus" | "external" | "dot";

const gear = "m9 2-.6 2.4-1.5.9-2.4-.7-3 5.2 1.8 1.7v1.8l-1.8 1.7 3 5.2 2.4-.7 1.5.9L9 23h6l.6-2.6 1.5-.9 2.4.7 3-5.2-1.8-1.7v-1.8l1.8-1.7-3-5.2-2.4.7-1.5-.9L15 2H9Zm7 10.5a4 4 0 1 1-8 0 4 4 0 0 1 8 0Z";
const paths: Record<IconName, string> = {
  back: "M10 3 1 12l9 9 2.5-2.5L8 14h14v-4H8l4.5-4.5L10 3Z",
  gear,
  settings: gear,
  devices: "M8 2h8a5 5 0 0 1 5 5v10a5 5 0 0 1-5 5H8a5 5 0 0 1-5-5V7a5 5 0 0 1 5-5Zm3 0v7H3v2h18V9h-8V2h-2Z",
  profiles: "m12 2 10 5-10 5L2 7l10-5ZM2 10v3l10 5 10-5v-3l-10 5-10-5Zm0 6v3l10 5 10-5v-3l-10 5-10-5Z",
  registry: "M5 2h14a2 2 0 0 1 2 2v16a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2Zm2 4v2h10V6H7Zm0 5v2h10v-2H7Zm0 5v2h7v-2H7Z",
  edit: "m17 2 5 5-3 3-5-5 3-3ZM12.5 6.5l5 5L8 21l-6 1 1-6 9.5-9.5Z",
  spark: "m12 1 3 8 8 3-8 3-3 8-3-8-8-3 8-3 3-8Z",
  check: "m9 16 11-11 3 3L9 22 1 14l3-3 5 5Z",
  "chevron-down": "m3 6 9 9 9-9 3 3-12 12L0 9l3-3Z",
  "chevron-up": "m3 18 9-9 9 9 3-3L12 3 0 15l3 3Z",
  close: "m5 2 7 7 7-7 3 3-7 7 7 7-3 3-7-7-7 7-3-3 7-7-7-7 3-3Z",
  plus: "M10 2h4v8h8v4h-8v8h-4v-8H2v-4h8V2Z",
  external: "M11 2h11v11h-4V9L8 19l-3-3L15 6h-4V2ZM2 6h6v4H6v10h10v-2h4v4H2V6Z",
  dot: "M20 12a8 8 0 1 1-16 0 8 8 0 0 1 16 0Z",
};

export function AppIcon({ name }: { name: IconName }) {
  return <svg className="app-icon" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true" focusable="false">
    <path d={paths[name]} fillRule="evenodd" clipRule="evenodd" />
  </svg>;
}
