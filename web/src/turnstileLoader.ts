const TURNSTILE_SCRIPT_ID = 'folio-turnstile-script';
const TURNSTILE_SCRIPT_URL = 'https://challenges.cloudflare.com/turnstile/v0/api.js';
const DEFAULT_TIMEOUT_MS = 10_000;

let loadingPromise: Promise<void> | null = null;

export function loadTurnstileScript(timeoutMs = DEFAULT_TIMEOUT_MS): Promise<void> {
  if (window.turnstile) return Promise.resolve();
  if (loadingPromise) return loadingPromise;

  document.getElementById(TURNSTILE_SCRIPT_ID)?.remove();

  const script = document.createElement('script');
  script.id = TURNSTILE_SCRIPT_ID;
  script.src = TURNSTILE_SCRIPT_URL;
  script.async = true;
  script.defer = true;

  let resolvePromise!: () => void;
  let rejectPromise!: (error: Error) => void;
  const promise = new Promise<void>((resolve, reject) => {
    resolvePromise = resolve;
    rejectPromise = reject;
  });
  loadingPromise = promise;

  let settled = false;
  let timeoutId: ReturnType<typeof setTimeout> | undefined;
  const settle = (error?: Error) => {
    if (settled) return;
    settled = true;
    if (timeoutId !== undefined) clearTimeout(timeoutId);
    script.onload = null;
    script.onerror = null;
    if (loadingPromise === promise) loadingPromise = null;

    if (error) {
      script.remove();
      rejectPromise(error);
    } else {
      resolvePromise();
    }
  };

  script.onload = () => {
    if (window.turnstile) settle();
    else settle(new Error('Turnstile script loaded without exposing its API'));
  };
  script.onerror = () => settle(new Error('Failed to load Turnstile script'));

  try {
    timeoutId = setTimeout(
      () => settle(new Error('Turnstile script load timed out')),
      timeoutMs,
    );
    document.head.appendChild(script);
  } catch (error) {
    settle(error instanceof Error ? error : new Error('Failed to append Turnstile script'));
  }

  return promise;
}
