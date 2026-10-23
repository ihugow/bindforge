import { test } from 'node:test';
import assert from 'node:assert/strict';
import { loadInventory, batteryLabel, watchInventory } from './devices.ts';
const settle = async () => { for (let i = 0; i < 6; i++) await Promise.resolve(); };

test('distinguishes a successful empty list from a failed query', async () => {
  const states = [];
  loadInventory(async () => [], state => states.push(state));
  await settle();
  assert.deepEqual(states.at(-1), { phase: 'ready', devices: [] });
  loadInventory(async () => { throw 'Restart the daemon'; }, state => states.push(state));
  await settle();
  assert.deepEqual(states.at(-1), { phase: 'error', message: 'Restart the daemon' });
});

test('ignores late results from an older refresh or disconnected daemon', async () => {
  let finish;
  const states = [];
  const cancel = loadInventory(() => new Promise(resolve => { finish = resolve; }), state => states.push(state));
  await settle(); cancel();
  loadInventory(async () => [{ id: 'new' }], state => states.push(state));
  await settle();
  finish([{ id: 'old' }]);
  await settle();
  assert.deepEqual(states.at(-1), { phase: 'ready', devices: [{ id: 'new' }] });
});

test('battery labels preserve zero, charging and unavailable readings', () => {
  assert.equal(batteryLabel({ battery: { percentage: 28, state: 'discharging' } }), 'Bateria: 28%');
  assert.equal(batteryLabel({ battery: { percentage: 0, state: 'charging' } }), 'Bateria: 0% · Carregando');
  assert.equal(batteryLabel({ battery: { percentage: 100, state: 'full' } }), 'Bateria: 100% · Carga completa');
  for (const device of [undefined, {}, { battery: null }, { battery: { percentage: 101 } }]) assert.equal(batteryLabel(device), 'Bateria indisponível');
});

test('periodic battery snapshots refresh and stop after cleanup', async context => {
  context.mock.timers.enable({ apis: ['setInterval'] });
  let reads = 0;
  const states = [];
  const stop = watchInventory(async () => [{ id: 'mouse', battery: { percentage: ++reads, state: 'discharging' } }], state => states.push(state));
  try {
    await settle();
    assert.equal(states.at(-1).devices[0].battery.percentage, 1);
    context.mock.timers.tick(60_000); await settle();
    assert.equal(states.at(-1).devices[0].battery.percentage, 2);
    stop();
    context.mock.timers.tick(60_000); await settle();
    assert.equal(reads, 2);
  } finally { stop(); }
});
