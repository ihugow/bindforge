import test from 'node:test';
import assert from 'node:assert/strict';
import { integrationAction, integrationReady, integrationSummary } from './kwin-integration.ts';
const ready = {kwin_available:true, daemon_connected:true, loaded:true, focus_received:true, legacy_disabled:false, message:''};
test('automatic integration requires a daemon, temporary script and authenticated focus', () => {
  assert.equal(integrationReady(ready), true);
  for (const key of ['kwin_available', 'daemon_connected', 'loaded', 'focus_received']) assert.equal(integrationReady({...ready, [key]:false}), false, key);
  assert.match(integrationSummary({...ready, focus_received:false}), /aguardando foco/);
  assert.match(integrationSummary({...ready, daemon_connected:false}), /Inicie o daemon/);
  assert.match(integrationSummary({...ready, loaded:false}), /tentando conectar/);
  assert.match(integrationSummary({...ready, kwin_available:false}), /Aguardando KDE/);
});
test('diagnostic offers reconnect and never installation or persistent enablement', () => {
  assert.equal(integrationAction(), 'Reconectar ao KDE');
  assert.equal(integrationReady({...ready, legacy_disabled:true}), true);
});
