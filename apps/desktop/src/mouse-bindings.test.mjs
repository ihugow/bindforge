import test from "node:test";
import assert from "node:assert/strict";
import { mouseBinding, setMouseBinding, configWithBindings, keyChord } from "./mouse-bindings.ts";
const general = { device_id: null, input: { MouseButton: 8 }, action: { KeyChord: ["V"] } };
const specific = { device_id: "mouse-a", input: { MouseButton: 8 }, action: "Disabled" };
test("specific mouse actions override general ones and reset restores the general mapping", () => {
  const bindings = [general, specific, { device_id: null, input: { Key: "A" }, action: { KeyChord: ["B"] } }];
  assert.equal(mouseBinding(bindings, "mouse-a", 8, true), specific);
  assert.equal(mouseBinding(bindings, "mouse-b", 8, true), general);
  const reset = setMouseBinding(bindings, "mouse-a", 8, null);
  assert.equal(mouseBinding(reset, "mouse-a", 8, true), general);
  assert.equal(reset.length, 2); assert.equal(bindings.length, 3);
  assert.equal(setMouseBinding(bindings, null, 8, { KeyChord: ["V"] }), bindings);
  const changed = setMouseBinding(bindings, null, 8, { KeyChord: ["Ctrl", "W"] });
  assert.equal(mouseBinding(changed, "mouse-a", 8), specific);
  assert.deepEqual(changed.find(b => "Key" in b.input), bindings[2]);
});
test("saving mouse bindings preserves profile metadata and rejects stale revisions", () => {
  const p = { id: "game", name: "Game", rules: [{ app_id: "game.exe" }], bindings: [general], enabled: true, priority: 10 };
  const config = { version: 1, revision: 3, fallback_id: "default", profiles: [{ ...p, id: "default", rules: [] }, p] };
  const next = configWithBindings(config, { ...p, name: "old name", rules: [], bindings: [specific] }, 3);
  assert.equal(next.profiles[1].name, "Game"); assert.deepEqual(next.profiles[1].rules, p.rules);
  assert.deepEqual(next.profiles[1].bindings, [specific]); assert.equal(config.profiles[1].bindings[0], general);
  assert.deepEqual(next.profiles[0], config.profiles[0]);
  assert.throws(() => configWithBindings(config, p, 2), /mudaram/);
  assert.throws(() => configWithBindings(config, { ...p, id: "removed" }, 3), /removido/);
});
test("keyboard chords accept single keys and reject empty, repeated or oversized combinations", () => {
  assert.deepEqual(keyChord("V"), { KeyChord: ["V"] });
  assert.deepEqual(keyChord(" Ctrl + Shift + V "), { KeyChord: ["Ctrl", "Shift", "V"] });
  for (const keys of ["", "Ctrl +", "Ctrl + ctrl", "x".repeat(129), Array.from({ length: 17 }, (_, i) => `K${i}`).join("+")]) {
    assert.throws(() => keyChord(keys));
  }
  assert.throws(() => keyChord("A\u0000B"));
});
