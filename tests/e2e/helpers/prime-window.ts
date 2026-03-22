/**
 * Test-only properties attached to `window` by Playwright `addInitScript` / patched WebSocket.
 */
export type PrimeTestWindow = Window & {
  __primeWsUpdate?: number;
  __primeWsSendPatched?: boolean;
  __primeWsUpdateTimes?: number[];
  __primeWsSendPatchedInterval?: boolean;
  __primeUpdateSkillSends?: number[];
};
