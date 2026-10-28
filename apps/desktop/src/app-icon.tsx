type IconName = "devices" | "profiles" | "registry" | "settings" | "gear" | "spark" | "edit";

const paths: Record<IconName, string> = {
  gear: "m9 3-.5 2a8 8 0 0 0-1.4.8L5.1 5.2l-3 5.2L3.6 12a8 8 0 0 0 0 1.6l-1.5 1.6 3 5.2 2-.6a8 8 0 0 0 1.4.8L9 22h6l.5-2a8 8 0 0 0 1.4-.8l2 .6 3-5.2-1.5-1.6a8 8 0 0 0 0-1.6l1.5-1.6-3-5.2-2 .6a8 8 0 0 0-1.4-.8L15 3H9Zm6.5 9.5a3.5 3.5 0 1 1-7 0 3.5 3.5 0 0 1 7 0Z",
  devices: "M8 3h8a4 4 0 0 1 4 4v10a4 4 0 0 1-4 4H8a4 4 0 0 1-4-4V7a4 4 0 0 1 4-4Zm4 0v7m-8 0h16",
  profiles: "m12 3 9 5-9 5-9-5 9-5Zm-9 9 9 5 9-5M3 16l9 5 9-5",
  registry: "M4 5h16v14H4V5Zm4 4h8m-8 4h5m-5 4h8",
  settings: "M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8Zm0-5v2m0 14v2M3 12h2m14 0h2M5.6 5.6 7 7m10 10 1.4 1.4M5.6 18.4 7 17m10-10 1.4-1.4",
  edit: "m16 4 4 4M4 16 16 4a2.8 2.8 0 0 1 4 4L8 20H4v-4Z",
  spark: "m12 3 2.4 6.6L21 12l-6.6 2.4L12 21l-2.4-6.6L3 12l6.6-2.4L12 3Z",
};

export function AppIcon({ name }: { name: IconName }) {
  return <svg className="app-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
    <path d={paths[name]} />
  </svg>;
}
