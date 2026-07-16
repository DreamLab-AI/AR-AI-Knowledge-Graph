/**
 * Bounded failure tracker for the post-processing render loop.
 *
 * The WebGPU render path (three.js WebGPUBackend) can throw transiently while
 * the canvas GPU context is lost/restored, and permanently when a backend
 * cannot support the node-based post-processing pipeline at all. A naive render
 * loop re-throws every frame, producing an unbounded stream of identical
 * uncaught errors (observed: 581 in a single session).
 *
 * This tracker converts that into: one structured log line per failure streak,
 * a bounded number of retries, then a terminal "disabled" decision so the
 * subsystem stops retrying and hands rendering back to the default renderer.
 *
 * Kept as a pure module (no React, no three.js) so the terminal-state logic is
 * unit-testable in isolation.
 */

export interface RenderGuardState {
  /** Number of consecutive failed frames since the last successful frame. */
  consecutiveFailures: number;
  /** Terminal flag — once true the subsystem must not retry again. */
  disabled: boolean;
}

export interface RenderGuardConfig {
  /** Consecutive failures tolerated before entering the terminal disabled state. */
  maxConsecutiveFailures: number;
}

export interface RenderFailureOutcome {
  /**
   * Emit a single structured log line. True only for the first failure of a
   * streak, so transient context-loss blips (which recover on the next good
   * frame) log at most once rather than every frame.
   */
  logFirstFailure: boolean;
  /**
   * The streak reached the ceiling — the caller must enter the terminal state
   * (dispose the pipeline, relinquish the render loop). True at most once.
   */
  disableNow: boolean;
}

export function initRenderGuard(): RenderGuardState {
  return { consecutiveFailures: 0, disabled: false };
}

/** Record a successful render frame — clears the transient failure streak. */
export function recordRenderSuccess(state: RenderGuardState): void {
  state.consecutiveFailures = 0;
}

/**
 * Record a failed render frame and decide whether to log and/or disable.
 * Mutates `state` in place (it is intended to live in a React ref).
 */
export function recordRenderFailure(
  state: RenderGuardState,
  config: RenderGuardConfig,
): RenderFailureOutcome {
  const logFirstFailure = state.consecutiveFailures === 0 && !state.disabled;
  state.consecutiveFailures += 1;

  const disableNow =
    !state.disabled && state.consecutiveFailures >= config.maxConsecutiveFailures;
  if (disableNow) {
    state.disabled = true;
  }

  return { logFirstFailure, disableNow };
}
