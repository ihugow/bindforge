import type { Rule } from "./profiles";

// Only explicit game identities can select artwork. Names and generic Gamescope
// associations are not reliable enough to infer a Steam game.
export function steamApps(rules: Pick<Rule, "app_id">[]): string[] {
  return [...new Set(rules.flatMap(({ app_id }) => {
    const value = /^steam:([1-9]\d{0,9})$/.exec(app_id ?? "")?.[1];
    return value && Number(value) <= 0xffffffff ? [`steam:${value}`] : [];
  }))];
}
export type SteamCover = { app_id: string; image: string };
export type FindSteamCovers = (appIds: string[]) => Promise<SteamCover[]>;
export function coverFor(rules: Pick<Rule, "app_id">[], covers: SteamCover[]): string | undefined {
  for (const id of steamApps(rules)) {
    const cover = covers.find(cover => cover.app_id === id && cover.image.startsWith("data:image/jpeg;base64,"));
    if (cover) return cover.image;
  }
}
