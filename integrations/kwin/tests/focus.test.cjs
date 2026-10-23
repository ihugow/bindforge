const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const script = fs.readFileSync(require('node:path').join(__dirname, '../contents/code/main.js'), 'utf8');

test('publishes initial focus, changes and no-focus using the D-Bus contract', () => {
  const calls = [];
  let onFocus;
  vm.runInNewContext(script, {
    workspace: {
      activeWindow: { desktopFileName: 'firefox', resourceClass: 'Firefox', pid: 123, caption: 'private title' },
      windowActivated: { connect(handler) { onFocus = handler; } },
    },
    callDBus(...args) { calls.push(args); },
  });
  onFocus({ resourceClass: 'org.kde.konsole', pid: -1 });
  onFocus(null);
  assert.deepEqual(calls, [
    ['org.appbuttons.Remap', '/org/appbuttons/Remap', 'org.appbuttons.Remap.Focus1', 'FocusChangedWithProcess', 'firefox', 'Firefox', '123'],
    ['org.appbuttons.Remap', '/org/appbuttons/Remap', 'org.appbuttons.Remap.Focus1', 'FocusChangedWithProcess', '', 'org.kde.konsole', '0'],
    ['org.appbuttons.Remap', '/org/appbuttons/Remap', 'org.appbuttons.Remap.Focus1', 'FocusChangedWithProcess', '', '', '0'],
  ]);
});
