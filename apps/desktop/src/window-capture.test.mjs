import test from "node:test";
import assert from "node:assert/strict";
import { captureWindow } from "./window-capture.ts";
const flush = () => new Promise(resolve => setImmediate(resolve));
test("waits for arming, delivers one ID and ignores later events", async () => {
  let receive; let stops = 0; const states = []; const ids = [];
  const dispose = captureWindow(async cb => { receive = cb; return () => stops++; }, s => states.push(s.phase), id => ids.push(id));
  assert.deepEqual(states, ["arming"]); await flush();
  assert.deepEqual(states, ["arming", "waiting"]);
  receive({ kind: "captured", app_id: "firefox" });
  receive({ kind: "captured", app_id: "remap-desktop" });
  dispose(); assert.deepEqual(ids, ["firefox"]); assert.equal(stops, 1);
});
test("cancels a late start and never delivers stale results", async () => {
  let resolve; let receive; let stops = 0; const ids = [];
  const dispose = captureWindow(cb => { receive = cb; return new Promise(r => resolve = r); }, () => {}, id => ids.push(id));
  await flush(); dispose(); receive({ kind: "captured", app_id: "stale" });
  resolve(() => stops++); await flush();
  assert.deepEqual(ids, []); assert.equal(stops, 1);
});
test("handles results before the start response and reports startup errors", async () => {
  const states = []; let stops = 0;
  captureWindow(async receive => { receive({ kind: "captured", app_id: "org.kde.konsole" }); return () => stops++; }, s => states.push(s.phase), () => {});
  await flush(); assert.deepEqual(states, ["arming", "captured"]); assert.equal(stops, 1);
  const errors = [];
  captureWindow(async () => { throw new Error("Reinicie o daemon"); }, s => errors.push(s), () => {});
  await flush(); assert.equal(errors.at(-1).phase, "error");
});
test("timeout and malformed IDs do not change the draft", async () => {
  for (const event of [{ kind: "error", message: "Tempo esgotado" }, { kind: "captured", app_id: "bad\nid" }]) {
    const states = []; const ids = [];
    captureWindow(async receive => { receive(event); return () => {}; }, s => states.push(s), id => ids.push(id));
    await flush(); assert.equal(states.at(-1).phase, "error"); assert.deepEqual(ids, []);
  }
});
test("ambiguous Gamescope is rejected while distinct Steam games are accepted", async () => {
  for (const [app, expected] of [["gamescope", null], ["Gamescope", null], ["steam:1808500", "steam:1808500"], ["steam:570", "steam:570"]]) {
    const ids = []; const states = [];
    captureWindow(async receive => { receive({ kind: "captured", app_id: app }); return () => {}; }, s => states.push(s), id => ids.push(id));
    await flush();
    assert.deepEqual(ids, expected ? [expected] : []);
    assert.equal(states.at(-1).phase, expected ? "captured" : "error");
  }
});
