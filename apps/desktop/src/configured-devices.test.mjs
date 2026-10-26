import test from 'node:test';
import assert from 'node:assert/strict';
import { withDevice, bindingScope, connectedInterface } from './configured-devices.ts';
import { mouseControls } from './mouse-bindings.ts';
const mouse = { id: 'custom', name: ' G PRO ', kind: 'mouse', interface_id: 'usb-a', button_count: 5, layout: null };
const detected = [{ id: 'usb-a', kind: 'mouse' }, { id: 'usb-b', kind: 'composite' }, { id: 'keyboard', kind: 'keyboard' }];
const config = { version: 1, revision: 2, fallback_id: 'default', profiles: [{ id: 'default', name: 'Padrão', bindings: [] }] };
test('registration preserves profiles and validates interface, count, layout and duplicates', () => {
  const next = withDevice(config, mouse, detected);
  assert.equal(next.devices[0].name, 'G PRO');
  assert.deepEqual(next.profiles, config.profiles);
  assert.equal(config.devices, undefined);
  assert.throws(() => withDevice(config, { ...mouse, interface_id: 'keyboard' }, detected), /compatível/);
  assert.throws(() => withDevice(config, { ...mouse, button_count: 2 }, detected), /propriedades/);
  assert.throws(() => withDevice(config, { ...mouse, button_count: 33 }, detected), /propriedades/);
  assert.throws(() => withDevice(next, { ...mouse, id: 'second' }, detected), /associada/);
  const keyboard = { ...mouse, id: 'keys', kind: 'keyboard', interface_id: 'keyboard', button_count: null, layout: 'abnt2' };
  assert.equal(withDevice(next, keyboard, detected).devices.length, 2);
  assert.throws(() => withDevice(config, { ...keyboard, layout: 'invalid' }, detected), /propriedades/);
});
test('relink moves only matching input kind, retains offline configuration and rejects destination conflicts', () => {
  const m = { device_id: 'usb-a', input: { MouseButton: 8 }, action: 'Disabled' };
  const k = { device_id: 'usb-a', input: { Key: 'A' }, action: 'Disabled' };
  const base = { ...config, devices: [mouse], profiles: [{ ...config.profiles[0], bindings: [m, k] }] };
  const moved = withDevice(base, { ...mouse, interface_id: 'usb-b' }, detected);
  assert.deepEqual(moved.profiles[0].bindings, [{ ...m, device_id: 'device:custom' }, k]);
  assert.equal(withDevice(base, { ...mouse, name: 'Offline' }, []).devices[0].name, 'Offline');
  const collision = { ...base, profiles: [{ ...base.profiles[0], bindings: [m, { ...m, device_id: 'usb-b' }] }] };
  assert.throws(() => withDevice(collision, { ...mouse, interface_id: 'usb-b' }, detected), /já possui ações/);
  assert.equal(base.profiles[0].bindings[0].device_id, 'usb-a');
});
test('mouse control count preserves established symbolic IDs and generates unique additional controls', () => {
  assert.deepEqual(mouseControls(5).map(b => b.code), [1, 3, 2, 8, 9]);
  assert.equal(mouseControls(3).length, 3);
  assert.equal(mouseControls(32).length, 32);
  assert.equal(new Set(mouseControls(32).map(b => b.code)).size, 32);
});

test('receiver and cable share actions across reconnects and removal without copying bindings', () => {
  const m = { device_id: 'usb-a', input: { MouseButton: 8 }, action: 'Disabled' };
  const old = { ...config, devices: [mouse], profiles: [{ ...config.profiles[0], bindings: [m] }] };
  const connections = [{ interface_id: 'usb-b', name: 'Cabo', fingerprint: 'cable' }, { interface_id: 'usb-a', name: 'Receptor', fingerprint: 'receiver' }];
  const next = withDevice(old, { ...mouse, connections }, detected);
  const device = next.devices[0];
  assert.equal(bindingScope(device), 'device:custom');
  assert.equal(next.profiles[0].bindings[0].device_id, bindingScope(device));
  assert.equal(connectedInterface(device, [{ id: 'new-path', kind: 'mouse', mouse_fingerprint: 'cable' }]).id, 'new-path');
  assert.equal(connectedInterface(device, [{ id: 'usb-b', kind: 'mouse', mouse_fingerprint: 'other' }]), undefined);
  assert.equal(connectedInterface(device, [{ id: 'one', kind: 'mouse', mouse_fingerprint: 'cable' }, { id: 'two', kind: 'mouse', mouse_fingerprint: 'cable' }]), undefined);
  const unplugged = withDevice(next, { ...device, connections: [connections[1]] }, []);
  assert.deepEqual(unplugged.profiles, next.profiles);
  assert.throws(() => withDevice(next, { ...device, id: 'another' }, detected), /associada/);
  assert.throws(() => withDevice(next, { ...device, connections: [connections[0], connections[0]] }, detected), /distintas/);
});
