import { describe, it, expect } from 'vitest';
import {
  initRenderGuard,
  recordRenderFailure,
  recordRenderSuccess,
} from '../postProcessingGuard';

const config = { maxConsecutiveFailures: 8 };

describe('postProcessingGuard', () => {
  it('starts healthy: no failures, not disabled', () => {
    const state = initRenderGuard();
    expect(state.consecutiveFailures).toBe(0);
    expect(state.disabled).toBe(false);
  });

  it('logs only the first failure of a streak', () => {
    const state = initRenderGuard();
    expect(recordRenderFailure(state, config).logFirstFailure).toBe(true);
    expect(recordRenderFailure(state, config).logFirstFailure).toBe(false);
    expect(recordRenderFailure(state, config).logFirstFailure).toBe(false);
  });

  it('does not disable before the ceiling', () => {
    const state = initRenderGuard();
    for (let i = 0; i < config.maxConsecutiveFailures - 1; i++) {
      expect(recordRenderFailure(state, config).disableNow).toBe(false);
    }
    expect(state.disabled).toBe(false);
  });

  it('enters the terminal disabled state exactly at the ceiling, once', () => {
    const state = initRenderGuard();
    let disableEvents = 0;
    for (let i = 0; i < config.maxConsecutiveFailures + 5; i++) {
      if (recordRenderFailure(state, config).disableNow) disableEvents++;
    }
    expect(state.disabled).toBe(true);
    expect(disableEvents).toBe(1);
  });

  it('a successful frame resets the streak so transient blips never disable', () => {
    const state = initRenderGuard();
    // Seven failures then a good frame, repeated — never reaches the ceiling.
    for (let cycle = 0; cycle < 5; cycle++) {
      for (let i = 0; i < config.maxConsecutiveFailures - 1; i++) {
        recordRenderFailure(state, config);
      }
      recordRenderSuccess(state);
      expect(state.consecutiveFailures).toBe(0);
    }
    expect(state.disabled).toBe(false);
  });

  it('logFirstFailure fires again after recovery (new streak)', () => {
    const state = initRenderGuard();
    expect(recordRenderFailure(state, config).logFirstFailure).toBe(true);
    recordRenderSuccess(state);
    expect(recordRenderFailure(state, config).logFirstFailure).toBe(true);
  });

  it('stops logging/disabling decisions once terminally disabled', () => {
    const state = initRenderGuard();
    for (let i = 0; i < config.maxConsecutiveFailures; i++) {
      recordRenderFailure(state, config);
    }
    expect(state.disabled).toBe(true);
    const after = recordRenderFailure(state, config);
    expect(after.logFirstFailure).toBe(false);
    expect(after.disableNow).toBe(false);
  });
});
