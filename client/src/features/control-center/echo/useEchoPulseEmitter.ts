/**
 * useEchoPulseEmitter — commit-time producer hook.
 *
 * Wraps `emitEchoPulse` with the Echo Pulse feature-flag gate
 * (`useControlCenterUI().echoPulseEnabled`, owned by WP2). `emitEchoPulse`
 * itself already no-ops under `prefers-reduced-motion` at the source level
 * (see echoPulseBus.ts) — this hook layers the user-facing store toggle on
 * top so either switch alone is enough to silence pulses.
 *
 * Producers call the returned function on COMMIT only (pointerup/change),
 * never per drag tick — see design-spec.md §4.4. Exported so future richer
 * producers (e.g. node-selection origins, not just camera-center) can reuse
 * the same gate without re-deriving it.
 */

import { useCallback } from 'react';
import { emitEchoPulse, type EchoPulseDetail } from './echoPulseBus';
import { useControlCenterUI } from '../state/useControlCenterUI';

export type EmitEchoPulse = (detail: Omit<EchoPulseDetail, 'ts'>) => void;

export function useEchoPulseEmitter(): EmitEchoPulse {
  const echoPulseEnabled = useControlCenterUI((s) => s.echoPulseEnabled);

  return useCallback(
    (detail: Omit<EchoPulseDetail, 'ts'>) => {
      if (!echoPulseEnabled) return;
      emitEchoPulse(detail);
    },
    [echoPulseEnabled],
  );
}
