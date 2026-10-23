import { test } from 'node:test';
import assert from 'node:assert/strict';
import { watchDaemon } from './daemon-status.ts';
const settle = async () => { for (let i = 0; i < 5; i++) await Promise.resolve(); };
const snapshot = (id, revision) => ({ protocol_version: 1, simulation: true,
  active_profile_id: id, focus_state: 'focused', revision,
  recent_profiles: [], history_available: true });

function source() {
  let receive;
  let disposed = 0;
  let starts = 0;
  return {
    subscribe: async callback => { receive = callback; starts++; return () => { disposed++; }; },
    send: event => receive(event), starts: () => starts, disposed: () => disposed,
  };
}

test('events clear stale status and reconnect with a new daemon revision', async () => {
  const stream = source(); const states = [];
  const stop = watchDaemon(stream.subscribe, state => states.push(state));
  await settle();
  stream.send({ kind: 'status', status: snapshot('firefox', '42') });
  assert.equal(states.at(-1).status.active_profile_id, 'firefox');
  stream.send({ kind: 'unavailable', message: 'Disconnected' });
  assert.deepEqual(states.at(-1), { phase: 'unavailable', message: 'Disconnected' });
  stream.send({ kind: 'status', status: snapshot('default', '1') });
  assert.equal(states.at(-1).status.revision, '1');
  assert.equal(stream.starts(), 1);
  stop(); assert.equal(stream.disposed(), 1);
});

test('late subscription startup is cleaned up after unmount and ignores stale events', async () => {
  let finish; let receive; let disposed = 0; const states = [];
  const stop = watchDaemon(callback => { receive = callback;
    return new Promise(resolve => { finish = resolve; });
  }, state => states.push(state));
  await settle();
  stop();
  receive({ kind: 'status', status: snapshot('firefox', '1') });
  finish(() => { disposed++; });
  await settle();
  assert.deepEqual(states, [{ phase: 'loading' }]);
  assert.equal(disposed, 1);
  stop(); assert.equal(disposed, 1);
});

test('preserves short profile visits in the daemon history', async () => {
  const stream = source(); const states = [];
  const stop = watchDaemon(stream.subscribe, state => states.push(state));
  await settle();
  stream.send({ kind: 'status', status: { ...snapshot('default', '3'), recent_profiles: [
    { revision: '1', profile_id: 'default' }, { revision: '2', profile_id: 'konsole' },
    { revision: '3', profile_id: 'default' },
  ] } });
  assert.deepEqual(states.at(-1).status.recent_profiles.map(change => change.profile_id),
    ['default', 'konsole', 'default']);
  stop();
});

test('delivers rapid updates directly without scheduling status polls', async () => {
  const stream = source(); const states = [];
  const stop = watchDaemon(stream.subscribe, state => states.push(state));
  await settle();
  for (let i = 1; i <= 20; i++) stream.send({ kind: 'status', status: snapshot('konsole', String(i)) });
  assert.equal(states.at(-1).status.revision, '20');
  assert.equal(states.length, 21);
  assert.equal(stream.starts(), 1);
  stop();
  stream.send({ kind: 'status', status: snapshot('default', '21') });
  assert.equal(states.length, 21);
});

test('failed subscription startup is visible without retaining a snapshot', async () => {
  const states = [];
  const stop = watchDaemon(async () => { throw new Error('invoke failed'); }, state => states.push(state));
  await settle();
  assert.equal(states.at(-1).phase, 'unavailable');
  assert.ok(!('status' in states.at(-1)));
  stop();
});
