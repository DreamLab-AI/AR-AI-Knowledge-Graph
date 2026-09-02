/**
 * Regression test for T2(a): doubled HTTP PUT per physics slider commit.
 *
 * Root cause (resolved 2026-06-03): physicsSlice.updatePhysics() called
 * notifyPhysicsUpdate(), which fired an IMMEDIATE settingsApi.updatePhysics()
 * (GET+PUT), while UnifiedSettingsTabContent.tsx → autoSaveManager.queueChange()
 * ALSO persisted via updateSettingsByPaths() after the 500 ms debounce. Two PUTs
 * per slider commit → double backend UpdateSimulationParams → double warmup reset
 * / double reheat.
 *
 * Fix: the debounced autoSaveManager path is the single canonical persistence
 * owner. notifyPhysicsUpdate no longer performs a network persistence side-effect;
 * it only leaves the local store state (already mutated in updatePhysics) in place.
 *
 * This test exercises the real physicsSlice in isolation and asserts:
 *   1. A slider commit (slice.updatePhysics) updates local store state.
 *   2. A slider commit fires ZERO network PUTs from the slice itself.
 *   3. The debounced autoSaveManager path is the sole persister (one PUT).
 *
 * Spec: docs/architecture/diagrams/04-updates-backoff.md
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

// ---------------------------------------------------------------------------
// Mock axios before any module that imports it (settingsApi / autoSaveManager).
// Each settingsApi.updatePhysics() does GET (current) + PUT (merged).
// ---------------------------------------------------------------------------
const mockGet = vi.fn().mockResolvedValue({ data: { repelK: 100 }, status: 200 });
const mockPut = vi.fn().mockResolvedValue({ data: {}, status: 200 });

vi.mock('axios', () => ({
  default: {
    get: (...args: unknown[]) => mockGet(...args),
    put: (...args: unknown[]) => mockPut(...args),
    interceptors: { request: { use: vi.fn() } },
  },
  get: (...args: unknown[]) => mockGet(...args),
  put: (...args: unknown[]) => mockPut(...args),
}));

import { createPhysicsSlice } from '../settings/physicsSlice';
import type { SettingsState, GPUPhysicsParams } from '../settings/settingsTypes';
import type { Settings } from '../../features/settings/config/settings';

/**
 * Build a minimal SettingsState harness that wires the real physics slice to a
 * fake store. updateSettings applies the recipe to a local draft so we can
 * observe the local-state mutation, exactly as the real coreSlice would.
 */
function buildSliceHarness() {
  const draft = { visualisation: { graphs: {} } } as unknown as Settings;

  const updateSettings = vi.fn((recipe: (d: Settings) => void) => {
    recipe(draft);
  });

  // get() returns the assembled state; the slice reads updateSettings and
  // notifyPhysicsUpdate from it.
  const state = {} as SettingsState;
  const get = () => state;
  const set = vi.fn();

  const slice = createPhysicsSlice(set as never, get as never, undefined as never);
  Object.assign(state, { updateSettings }, slice);

  return { state, draft, updateSettings, slice };
}

describe('REPRO T2(a): physics slider commit must produce exactly one PUT', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('slice.updatePhysics mutates local store state for the changed param', () => {
    const { state, draft } = buildSliceHarness();

    state.updatePhysics('knowledge', { repelK: 110 } as Partial<GPUPhysicsParams>);

    const graphs = (draft as unknown as {
      visualisation: { graphs: Record<string, { physics?: Record<string, unknown> }> };
    }).visualisation.graphs;
    expect(graphs.knowledge.physics).toMatchObject({ repelK: 110 });
  });

  it('slice.updatePhysics (via notifyPhysicsUpdate) fires ZERO network PUTs', () => {
    const { state } = buildSliceHarness();

    // A single slider commit.
    state.updatePhysics('knowledge', { repelK: 110 } as Partial<GPUPhysicsParams>);

    // The immediate persistence side-effect was removed: the slice must not
    // touch the network. Persistence is owned solely by the debounced
    // autoSaveManager path.
    expect(mockGet).toHaveBeenCalledTimes(0);
    expect(mockPut).toHaveBeenCalledTimes(0);
  });

  it('exactly one PUT per slider commit: only the debounced autoSave path persists', async () => {
    const { state } = buildSliceHarness();

    // Path that used to fire immediately — now a no-op for the network.
    state.updatePhysics('knowledge', { repelK: 110 } as Partial<GPUPhysicsParams>);
    expect(mockPut).toHaveBeenCalledTimes(0);

    // The canonical debounced path (autoSaveManager → updateSettingsByPaths →
    // settingsApi.updatePhysics) is the SOLE persister and fires exactly once.
    const { autoSaveManager } = await import('../autoSaveManager');
    autoSaveManager.setInitialized(true);
    autoSaveManager.queueChange('visualisation.graphs.knowledge.physics.repelK', 110);

    await vi.advanceTimersByTimeAsync(500);

    // One GET (current) + one PUT (merged) for the single physics change.
    expect(mockGet).toHaveBeenCalledTimes(1);
    expect(mockPut).toHaveBeenCalledTimes(1);
  });
});
