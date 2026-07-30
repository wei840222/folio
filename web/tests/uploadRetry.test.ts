import assert from 'node:assert/strict';
import test from 'node:test';

import {
  beginChallengeAttempt,
  canCancelChallenge,
  classifyChallengeFailure,
  isCurrentChallengeAttempt,
} from '../src/uploadRetry.ts';

test('hard rate limit exits the challenge flow', () => {
  assert.deepEqual(classifyChallengeFailure(429, 'rate_limited'), {
    retryChallenge: false,
    message: '上傳次數過多，請稍後再試。',
  });
});

test('resource failures exit the challenge flow', () => {
  assert.deepEqual(classifyChallengeFailure(429, 'upload_busy'), {
    retryChallenge: false,
    message: '目前上傳流量繁忙，請稍後再試。',
  });
  assert.deepEqual(classifyChallengeFailure(503, undefined), {
    retryChallenge: false,
    message: '伺服器暫時無法完成上傳，請稍後再試。',
  });
  assert.equal(classifyChallengeFailure(413, undefined).retryChallenge, false);
  assert.equal(classifyChallengeFailure(408, undefined).retryChallenge, false);
  assert.equal(classifyChallengeFailure(507, undefined).retryChallenge, false);
});

test('only verification failures request a fresh challenge token', () => {
  assert.equal(classifyChallengeFailure(403, undefined).retryChallenge, true);
  assert.equal(classifyChallengeFailure(429, 'challenge_required').retryChallenge, true);
});

test('an in-flight challenge attempt keeps its file and ignores stale responses', () => {
  const file = { name: 'report.pdf' };
  const attempt = beginChallengeAttempt(7, file);

  assert.deepEqual(attempt, { generation: 7, file });
  assert.equal(isCurrentChallengeAttempt(7, attempt), true);
  assert.equal(isCurrentChallengeAttempt(8, attempt), false);
  assert.equal(canCancelChallenge(true), false);
  assert.equal(canCancelChallenge(false), true);
});
