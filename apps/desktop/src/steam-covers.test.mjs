import test from 'node:test';
import assert from 'node:assert/strict';
import { steamApps, coverFor } from './steam-covers.ts';
const rules = (...ids) => ids.map(app_id => ({ app_id }));
test('artwork uses exact positive Steam u32 identities, never profile names or generic windows', () => {
  assert.deepEqual(steamApps(rules('steam:1808500', 'steam:570', 'steam:1808500', 'steam:4294967295')), ['steam:1808500', 'steam:570', 'steam:4294967295']);
  assert.deepEqual(steamApps(rules(null, 'gamescope', 'ARC Raiders', 'steam:0', 'steam:01', 'steam:-1', 'steam:4294967296', 'steam:../570', 'steam:570\n', 'steam:570oops')), []);
});
test('multi-game profiles use first associated available cover and reject remote or malformed images', () => {
  const image = 'data:image/jpeg;base64,fixture';
  const covers = [{ app_id: 'steam:570', image }, { app_id: 'steam:730', image: 'https://example.com/image.jpg' }];
  assert.equal(coverFor(rules('steam:1808500', 'steam:570'), covers), image);
  assert.equal(coverFor(rules('steam:730'), covers), undefined);
  assert.equal(coverFor(rules('firefox'), covers), undefined);
  assert.equal(coverFor(rules('steam:1808500'), covers), undefined);
});
