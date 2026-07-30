import assert from 'node:assert/strict';
import test from 'node:test';

import { readExpiresAt } from '../src/uploadResponse.ts';

function responseWith(body: unknown): Pick<Response, 'json'> {
  return {
    json: async () => body,
  };
}

test('reads a client-representable expiration timestamp', async () => {
  assert.equal(
    await readExpiresAt(responseWith({ expires_at: 253_402_300_799 })),
    253_402_300_799,
  );
});

test('invalid expiration data degrades to unavailable', async () => {
  const invalidBodies = [
    null,
    {},
    { expires_at: 'tomorrow' },
    { expires_at: Number.MAX_SAFE_INTEGER + 1 },
    { expires_at: 8_640_000_000_001 },
    { expires_at: -1 },
  ];

  for (const body of invalidBodies) {
    assert.equal(await readExpiresAt(responseWith(body)), null);
  }

  assert.equal(
    await readExpiresAt({
      json: async () => {
        throw new SyntaxError('invalid JSON');
      },
    }),
    null,
  );
});
