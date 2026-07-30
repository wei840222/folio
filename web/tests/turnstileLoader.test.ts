import assert from 'node:assert/strict';
import test from 'node:test';

import { loadTurnstileScript } from '../src/turnstileLoader.ts';

interface FakeScript {
  id: string;
  src: string;
  async: boolean;
  defer: boolean;
  onload: (() => void) | null;
  onerror: (() => void) | null;
  removed: boolean;
  remove(): void;
}

function fakeScript(): FakeScript {
  return {
    id: '',
    src: '',
    async: false,
    defer: false,
    onload: null,
    onerror: null,
    removed: false,
    remove() {
      this.removed = true;
    },
  };
}

async function withBrowserGlobals(
  fakeWindow: object,
  fakeDocument: object,
  run: () => Promise<void>,
): Promise<void> {
  const windowDescriptor = Object.getOwnPropertyDescriptor(globalThis, 'window');
  const documentDescriptor = Object.getOwnPropertyDescriptor(globalThis, 'document');
  Object.defineProperty(globalThis, 'window', { configurable: true, value: fakeWindow });
  Object.defineProperty(globalThis, 'document', { configurable: true, value: fakeDocument });
  try {
    await run();
  } finally {
    if (windowDescriptor) Object.defineProperty(globalThis, 'window', windowDescriptor);
    else delete (globalThis as { window?: unknown }).window;
    if (documentDescriptor) Object.defineProperty(globalThis, 'document', documentDescriptor);
    else delete (globalThis as { document?: unknown }).document;
  }
}

test('script errors settle the loader and allow a clean retry', async () => {
  const scripts: FakeScript[] = [];
  const fakeWindow: { turnstile?: object } = {};
  const fakeDocument = {
    getElementById: () => null,
    createElement: () => fakeScript(),
    head: {
      appendChild(script: FakeScript) {
        scripts.push(script);
        queueMicrotask(() => {
          if (scripts.length === 1) {
            script.onerror?.();
          } else {
            fakeWindow.turnstile = {};
            script.onload?.();
          }
        });
      },
    },
  };

  await withBrowserGlobals(fakeWindow, fakeDocument, async () => {
    await assert.rejects(loadTurnstileScript(100), /failed to load/i);
    assert.equal(scripts[0]?.removed, true);

    await loadTurnstileScript(100);
    assert.equal(scripts.length, 2);
  });
});

test('script timeouts settle the loader and remove the stalled script', async () => {
  const script = fakeScript();
  const fakeDocument = {
    getElementById: () => null,
    createElement: () => script,
    head: { appendChild() {} },
  };

  await withBrowserGlobals({}, fakeDocument, async () => {
    await assert.rejects(loadTurnstileScript(5), /timed out/i);
    assert.equal(script.removed, true);
  });
});
