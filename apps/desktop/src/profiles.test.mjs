import test from "node:test";
import assert from "node:assert/strict";
import { updateProfile, conflictingProfiles } from "./profiles.ts";
const config = () => ({ version: 1, revision: 3, fallback_id: "default", profiles: [
  { id: "default", name: "Padrão", enabled: true, priority: 0, rules: [], bindings: [{ device_id: null, input: { MouseButton: 8 }, action: { KeyChord: ["Alt", "Left"] } }] },
  { id: "browser", name: "Browser", enabled: true, priority: 10, rules: [{ app_id: "firefox", executable: null, title_contains: null }], bindings: [] },
] });
const draft = () => ({ id: null, name: "Trabalho", apps: "firefox\norg.kde.konsole\nfirefox", priority: "20", enabled: true, source: "default" });
test("creation copies independently, deduplicates apps, and reports overlap", () => {
  const c = config(); const d = draft();
  assert.deepEqual(conflictingProfiles(c, d).map(p => p.id), ["browser"]);
  const next = updateProfile(c, d, "new");
  assert.equal(next.revision, 3);
  assert.deepEqual(next.profiles[2].rules.map(r => r.app_id), ["firefox", "org.kde.konsole"]);
  next.profiles[2].bindings[0].action.KeyChord.push("Shift");
  assert.equal(c.profiles[0].bindings[0].action.KeyChord.length, 2);
  assert.equal(updateProfile(c, { ...d, source: "" }, "empty").profiles[2].bindings.length, 0);
});
test("editing preserves actions and fallback invariants", () => {
  const c = config();
  const next = updateProfile(c, { ...draft(), id: "default", enabled: false }, "unused");
  assert.equal(next.profiles.length, 2);
  assert.equal(next.profiles[0].enabled, true);
  assert.deepEqual(next.profiles[0].rules, []);
  assert.deepEqual(next.profiles[0].bindings, c.profiles[0].bindings);
  assert.throws(() => updateProfile(c, { ...draft(), apps: " " }, "new"));
  assert.throws(() => updateProfile(c, { ...draft(), priority: "1.5" }, "new"));
});
test("advanced predicates survive editing without app IDs", () => {
  const c = config(); c.profiles[1].rules = [{ app_id: null, executable: "/bin/editor", title_contains: "Work" }];
  const next = updateProfile(c, { ...draft(), id: "browser", apps: "", name: "Editor" }, "unused");
  assert.deepEqual(next.profiles[1].rules, c.profiles[1].rules);
});
test("deletion preserves the fallback and other profiles without mutating the loaded config", async () => {
  const { removeProfile } = await import("./profiles.ts");
  const c = config(); const next = removeProfile(c, "browser");
  assert.deepEqual(next.profiles.map(p => p.id), ["default"]);
  assert.deepEqual(next.profiles[0].bindings, c.profiles[0].bindings);
  assert.equal(c.profiles.length, 2);
  assert.equal(next.revision, c.revision, "the store owns revision changes");
  assert.throws(() => removeProfile(c, c.fallback_id), /Padrão/);
  assert.throws(() => removeProfile(c, "missing"), /não existe/);
});
test("profile icons survive edits and binding copies and can return to the default", () => {
  const icon = "data:image/png;base64,iVBORw0KGgo=";
  const c = config(); c.profiles[1].icon = icon;
  const edited = updateProfile(c, { ...draft(), id: "browser", icon }, "unused");
  assert.equal(edited.profiles[1].icon, icon);
  assert.equal(updateProfile(edited, { ...draft(), id: "browser" }, "unused").profiles[1].icon, icon);
  assert.equal(updateProfile(edited, { ...draft(), id: "browser", icon: null }, "unused").profiles[1].icon, null);
  assert.equal(updateProfile(c, { ...draft(), source: "browser" }, "new").profiles[2].icon, null, "copy actions without inheriting the source icon");
  assert.equal(c.profiles[1].icon, icon);
});
