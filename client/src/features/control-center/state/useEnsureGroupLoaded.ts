/**
 * useEnsureGroupLoaded — lazy per-group hydration. design-spec.md §4.2.
 *
 * Fixes documented gap #6: nothing in today's UI calls coreSlice.ensureLoaded,
 * so any field outside ESSENTIAL_PATHS reads `undefined` on a cold localStorage.
 * A group's coarse `loadPaths` are fetched the first time it opens.
 *
 * coreSlice.ensureLoaded is itself idempotent — it filters already-loaded paths
 * and short-circuits (no network) when nothing is missing — so re-opening a
 * group is a cheap no-op. We call it through `getState()` at invocation time so
 * a spy/stub installed in tests is always honoured.
 */

import { useCallback, useEffect, useRef } from 'react';
import { useSettingsStore } from '../../../store/settingsStore';
import { MACRO_PATHS } from '../registry/macros';
import type { RegistryGroup } from '../registry/types';

/**
 * Returns a stable callback that hydrates a group's coarse loadPaths. Wire it to
 * `GroupSection.onFirstMount`, which fires exactly once per mount — so opening a
 * group triggers exactly one ensureLoaded(group.loadPaths).
 */
export function useEnsureGroupLoaded(): (group: RegistryGroup) => void {
  return useCallback((group: RegistryGroup) => {
    if (!group?.loadPaths?.length) return;
    void useSettingsStore
      .getState()
      .ensureLoaded(group.loadPaths)
      .catch(() => {
        /* fail-open: a hydration miss leaves the row on its default, not broken */
      });
  }, []);
}

/**
 * MacroBar mount → hydrate the subtrees the five dials derive from. Several of
 * these (glow, gemMaterial, sceneEffects, labels, nodeFilter) sit outside
 * ESSENTIAL_PATHS, so without this the dials would sit at their fallback
 * positions on a cold load. Runs once.
 */
export function useEnsureMacroPathsLoaded(): void {
  const ran = useRef(false);
  useEffect(() => {
    if (ran.current) return;
    ran.current = true;
    void useSettingsStore
      .getState()
      .ensureLoaded(MACRO_PATHS)
      .catch(() => {
        /* fail-open */
      });
  }, []);
}
