import test from "node:test";
import assert from "node:assert/strict";
import { keyboardSections, keyboardKeyGroups } from "./keyboard-layouts.ts";
import { keyboardBinding, setKeyboardBinding } from "./keyboard-bindings.ts";

test("physical layouts preserve key identity and include their distinct keys and geometry", () => {
  const layouts = Object.fromEntries(["ansi", "iso", "abnt2"].map(layout => [layout, keyboardSections(layout).flatMap(section => section.keys)]));
  assert.equal(layouts.ansi.length, 104);
  assert.equal(layouts.iso.length, 105);
  assert.equal(layouts.abnt2.length, 107);
  for (const keys of Object.values(layouts)) assert.equal(new Set(keys.map(k => k.code)).size, keys.length);
  const find = (layout, code) => layouts[layout].find(k => k.code === code);
  assert.equal(find("abnt2", "KEY_SEMICOLON").label, "Ç");
  assert.equal(find("ansi", "KEY_SEMICOLON").label, "; :");
  assert.equal(find("ansi", "KEY_ENTER").height, 1);
  assert.equal(find("iso", "KEY_ENTER").height, 2);
  assert.equal(find("iso", "KEY_LEFTSHIFT").width, 1.25);
  assert.equal(find("ansi", "KEY_LEFTSHIFT").width, 2.25);
  assert.ok(find("abnt2", "KEY_RO"));
  assert.ok(find("abnt2", "KEY_KPCOMMA"));
  assert.equal(find("iso", "KEY_RO"), undefined);
  assert.equal(find("ansi", "KEY_102ND"), undefined);
});

test("keyboard edits preserve mouse actions, general actions and other keyboards", () => {
  const general = { device_id: null, input: { Key: "CapsLock" }, action: { KeyChord: ["Ctrl"] } };
  const mouse = { device_id: "keyboard-a", input: { MouseButton: 8 }, action: "Disabled" };
  const other = { device_id: "keyboard-b", input: { Key: "KEY_CAPSLOCK" }, action: "Disabled" };
  const old = { device_id: "keyboard-a", input: { Key: "CapsLock" }, action: { KeyChord: ["A"] } };
  const bindings = [general, mouse, other, old];
  const changed = setKeyboardBinding(bindings, "keyboard-a", "KEY_CAPSLOCK", { KeyChord: ["KEY_ESC"] });
  assert.deepEqual(changed.slice(0, 3), [general, mouse, other]);
  assert.equal(changed.length, bindings.length);
  assert.equal(bindings[3], old);
  assert.deepEqual(keyboardBinding(changed, "keyboard-a", "CapsLock", true).action, { KeyChord: ["KEY_ESC"] });
  const reset = setKeyboardBinding(changed, "keyboard-a", "CapsLock", null);
  assert.equal(keyboardBinding(reset, "keyboard-a", "KEY_CAPSLOCK"), undefined);
  assert.equal(keyboardBinding(reset, "keyboard-a", "KEY_CAPSLOCK", true), general);
  assert.equal(keyboardBinding(reset, "keyboard-b", "KEY_CAPSLOCK", true), other);
});

test("destination key groups cover all layouts without missing or duplicating keys", () => {
  for (const layout of ["ansi", "iso", "abnt2"]) {
    const groups = keyboardKeyGroups(layout);
    const original = keyboardSections(layout).flatMap(section => section.keys.map(key => key.code)).sort();
    assert.deepEqual(groups.flatMap(group => group.keys.map(key => key.code)).sort(), original);
    const codes = name => groups.find(group => group.name === name).keys.map(key => key.code);
    assert.equal(codes("Letras").length, 26);
    assert.equal(codes("Números").length, 10);
    assert.equal(codes("Funções").length, 12);
    assert.equal(codes("Modificadores").length, 8);
    assert.ok(codes("Controles").includes("KEY_ENTER"));
    assert.ok(codes("Teclado numérico").includes("KEY_KPENTER"));
  }
});
