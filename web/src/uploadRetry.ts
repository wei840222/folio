export interface ChallengeFailure {
  retryChallenge: boolean;
  message: string;
}

export interface ChallengeAttempt<T> {
  generation: number;
  file: T;
}

export function beginChallengeAttempt<T>(
  generation: number,
  pendingFile: T | null,
): ChallengeAttempt<T> | null {
  return pendingFile === null ? null : { generation, file: pendingFile };
}

export function isCurrentChallengeAttempt<T>(
  currentGeneration: number,
  attempt: ChallengeAttempt<T> | null,
): boolean {
  return attempt !== null && attempt.generation === currentGeneration;
}

export function canCancelChallenge(isUploading: boolean): boolean {
  return !isUploading;
}

export function classifyChallengeFailure(status: number, code: string | undefined): ChallengeFailure {
  if (status === 403 || (status === 429 && code === 'challenge_required')) {
    return {
      retryChallenge: true,
      message: '上傳驗證失敗，請重新完成驗證。',
    };
  }

  if (status === 429 && code === 'rate_limited') {
    return {
      retryChallenge: false,
      message: '上傳次數過多，請稍後再試。',
    };
  }

  if (status === 429 && code === 'upload_busy') {
    return {
      retryChallenge: false,
      message: '目前上傳流量繁忙，請稍後再試。',
    };
  }

  if (status === 413) {
    return {
      retryChallenge: false,
      message: '檔案或上傳資料超過大小限制。',
    };
  }

  if (status === 408) {
    return {
      retryChallenge: false,
      message: '上傳逾時，請重新選擇檔案後再試。',
    };
  }

  if (status === 507) {
    return {
      retryChallenge: false,
      message: '儲存空間不足，暫時無法上傳。',
    };
  }

  if (status >= 500) {
    return {
      retryChallenge: false,
      message: '伺服器暫時無法完成上傳，請稍後再試。',
    };
  }

  return {
    retryChallenge: false,
    message: '檔案上傳失敗，請確認檔案後再試。',
  };
}
