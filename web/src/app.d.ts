/// <reference types="svelte" />

declare module '*.css';

interface TurnstileApi {
  render(
    container: HTMLElement,
    options: {
      sitekey: string;
      action: string;
      callback: (token: string) => void;
    },
  ): string;
  reset(widgetId: string): void;
}

interface Window {
  turnstile?: TurnstileApi;
}
