import type { ConfiguredDevice } from "./configured-devices";

export type KeyboardLayout = NonNullable<ConfiguredDevice["layout"]>;
export type KeyboardKey = { code: string; label: string; x: number; y: number; width: number; height: number; isoEnter?: boolean };
export type KeyboardSection = { id: string; name: string; width: number; keys: KeyboardKey[] };

// Positions use key units; codes identify physical Linux keys, independently of legends.
// Geometry and Brazilian legends follow the system XKB pc/abnt2 definitions.
export function keyboardSections(layout: KeyboardLayout): KeyboardSection[] {
  const brazil = layout === "abnt2", iso = layout !== "ansi";
  const main: KeyboardKey[] = [], navigation: KeyboardKey[] = [], keypad: KeyboardKey[] = [];
  function add(keys: KeyboardKey[], code: string, label: string, x: number, y: number, width = 1, height = 1, isoEnter = false) {
    keys.push({ code: `KEY_${code}`, label, x, y, width, height, isoEnter });
  }
  function row(y: number, start: number, codes: string[], labels = codes) {
    codes.forEach((code, i) => add(main, code, labels[i], start + i, y));
  }
  add(main, "ESC", "Esc", 0, 0);
  for (let i = 1; i <= 12; i++) add(main, `F${i}`, `F${i}`, 1.75 + i - 1 + Math.floor((i - 1) / 4) * .5, 0);
  row(1, 0, ["GRAVE", "1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "MINUS", "EQUAL"], [brazil ? "' \"" : "` ~", "1 !", "2 @", "3 #", "4 $", "5 %", brazil ? "6 ¨" : "6 ^", "7 &", "8 *", "9 (", "0 )", "- _", "= +"]);
  add(main, "BACKSPACE", "Backspace", 13, 1, 2);
  add(main, "TAB", "Tab", 0, 2, 1.5);
  row(2, 1.5, [..."QWERTYUIOP", "LEFTBRACE", "RIGHTBRACE"], [..."QWERTYUIOP", brazil ? "´ `" : "[ {", brazil ? "[ {" : "] }"]);
  if (iso) add(main, "ENTER", "Enter", 13.5, 2, 1.5, 2, true);
  else add(main, "BACKSLASH", "\\ |", 13.5, 2, 1.5);
  add(main, "CAPSLOCK", "Caps Lock", 0, 3, 1.75);
  row(3, 1.75, [..."ASDFGHJKL", "SEMICOLON", "APOSTROPHE"], [..."ASDFGHJKL", brazil ? "Ç" : "; :", brazil ? "~ ^" : "' \""]);
  if (iso) add(main, "BACKSLASH", brazil ? "] }" : "# ~", 12.75, 3);
  else add(main, "ENTER", "Enter", 12.75, 3, 2.25);
  add(main, "LEFTSHIFT", "Shift", 0, 4, iso ? 1.25 : 2.25);
  if (iso) add(main, "102ND", brazil ? "\\ |" : "< >", 1.25, 4);
  row(4, 2.25, [..."ZXCVBNM", "COMMA", "DOT", "SLASH"], [..."ZXCVBNM", ", <", ". >", brazil ? "; :" : "/ ?"]);
  if (brazil) add(main, "RO", "/ ?", 12.25, 4);
  add(main, "RIGHTSHIFT", "Shift", brazil ? 13.25 : 12.25, 4, brazil ? 1.75 : 2.75);
  add(main, "LEFTCTRL", "Ctrl", 0, 5, 1.25);
  add(main, "LEFTMETA", "Super", 1.25, 5, 1.25);
  add(main, "LEFTALT", "Alt", 2.5, 5, 1.25);
  add(main, "SPACE", "Espaço", 3.75, 5, 6.25);
  add(main, "RIGHTALT", iso ? "AltGr" : "Alt", 10, 5, 1.25);
  add(main, "RIGHTMETA", "Super", 11.25, 5, 1.25);
  add(main, "COMPOSE", "Menu", 12.5, 5, 1.25);
  add(main, "RIGHTCTRL", "Ctrl", 13.75, 5, 1.25);
  ["SYSRQ", "SCROLLLOCK", "PAUSE"].forEach((code, x) => add(navigation, code, ["Print", "Scroll", "Pause"][x], x, 0));
  ["INSERT", "HOME", "PAGEUP", "DELETE", "END", "PAGEDOWN"].forEach((code, i) => add(navigation, code, ["Ins", "Home", "PgUp", "Del", "End", "PgDn"][i], i % 3, 1 + Math.floor(i / 3)));
  add(navigation, "UP", "↑", 1, 4);
  ["LEFT", "DOWN", "RIGHT"].forEach((code, x) => add(navigation, code, ["←", "↓", "→"][x], x, 5));
  ["NUMLOCK", "KPSLASH", "KPASTERISK", "KPMINUS"].forEach((code, x) => add(keypad, code, ["Num", "/", "*", "−"][x], x, 1));
  for (let y = 2; y <= 4; y++) for (let x = 0; x < 3; x++) {
    const number = (4 - y) * 3 + x + 1;
    add(keypad, `KP${number}`, String(number), x, y);
  }
  add(keypad, "KPPLUS", "+", 3, 2, 1, brazil ? 1 : 2);
  if (brazil) add(keypad, "KPCOMMA", ".", 3, 3);
  add(keypad, "KPENTER", "Enter", 3, 4, 1, 2);
  add(keypad, "KP0", "0", 0, 5, 2);
  add(keypad, "KPDOT", brazil ? "," : ".", 2, 5);
  return [{ id: "main", name: "Teclas principais", width: 15, keys: main }, { id: "navigation", name: "Navegação", width: 3, keys: navigation }, { id: "keypad", name: "Teclado numérico", width: 4, keys: keypad }];
}

export function keyName(key: KeyboardKey): string {
  const side = key.code.startsWith("KEY_LEFT") ? "esquerdo" : key.code.startsWith("KEY_RIGHT") ? "direito" : "";
  if (["Ctrl", "Shift", "Alt", "Super"].includes(key.label)) return `${key.label} ${side}`.trim();
  return key.code.startsWith("KEY_KP") ? `${key.label} (numérico)` : key.label;
}
