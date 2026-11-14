import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import { runInNewContext } from 'node:vm';

const html = readFileSync(new URL('../web/index.html', import.meta.url), 'utf8');
const bootstrap = html.match(/<script>([\s\S]*?)<\/script>/)[1];

test('audio unlock is installed before the wasm module and retries trusted gestures', async () => {
  assert.ok(html.indexOf('<script>') < html.indexOf('<script type="module">'));
  const listeners = new Map();
  class AudioContext {
    state = 'suspended';
    attempts = 0;
    denied = false;

    resume() {
      this.attempts += 1;
      if (this.denied) return Promise.reject(new Error('autoplay denied'));
      this.state = 'running';
      return Promise.resolve();
    }
  }
  const window = {
    AudioContext,
    webkitAudioContext: AudioContext,
    addEventListener(event, listener, options) {
      assert.equal(options.capture, true);
      listeners.set(event, listener);
    },
  };
  runInNewContext(bootstrap, { window });
  const primary = new window.AudioContext();
  const prefixed = new window.webkitAudioContext();
  assert.ok(primary instanceof AudioContext);
  assert.equal(primary.state, 'suspended');
  listeners.get('pointerdown')();
  assert.equal(primary.state, 'running');
  assert.equal(prefixed.state, 'running');

  primary.state = 'suspended';
  primary.denied = true;
  prefixed.state = 'closed';
  listeners.get('touchend')();
  await Promise.resolve();
  assert.equal(primary.attempts, 2);
  assert.equal(prefixed.attempts, 1);

  primary.denied = false;
  listeners.get('keydown')();
  assert.equal(primary.state, 'running');
  assert.equal(primary.attempts, 3);
});

test('browsers without Web Audio still reach the WebGPU fallback', () => {
  runInNewContext(bootstrap, { window: { addEventListener() {} } });
});
