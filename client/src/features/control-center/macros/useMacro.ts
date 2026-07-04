/**
 * useMacro — forward-write + derive-read binding for one L1 macro dial.
 * design-spec.md §1.1, §3.1, §4.4.
 *
 * - READ: `value` derives the dial's 0..1 position from the current settings via
 *   `macro.derive`, subscribed fine-grained (the selector returns a number, so
 *   the dial re-renders only when its derived position actually changes).
 * - WRITE: `onChange(t)` runs `macro.apply(t)` and pushes every resulting path
 *   through the SAME write path as useSettingField — one immer batch into the
 *   settings store, then an explicit per-path `autoSaveManager.queueChange`
 *   (mirroring useSettingField's deliberate redundancy). No new persistence.
 * - COMMIT: `onCommit(t)` re-applies and fires a single echo pulse (gated by the
 *   echoPulseEnabled UI flag), never per drag tick.
 */

import { useCallback } from 'react';
import type { MacroDef } from '../registry/types';
import { useSettingsStore } from '../../../store/settingsStore';
import { autoSaveManager } from '../../../store/autoSaveManager';
import { emitEchoPulse } from '../echo/echoPulseBus';
import { useControlCenterUI } from '../state/useControlCenterUI';

type MacroWrites = Array<{ path: string; value: number | boolean }>;

function getByPath(root: unknown, path: string): unknown {
  if (!path) return undefined;
  let cur: unknown = root;
  for (const key of path.split('.')) {
    if (cur == null || typeof cur !== 'object') return undefined;
    cur = (cur as Record<string, unknown>)[key];
  }
  return cur;
}

function applyMacroWrites(writes: MacroWrites): void {
  if (!writes.length) return;
  const store = useSettingsStore.getState();

  // Single immer batch: fans out to R3F once via the store's subscriber trie.
  store.updateSettings((draft) => {
    const root = draft as unknown as Record<string, unknown>;
    for (const { path, value } of writes) {
      const keys = path.split('.');
      let cur = root;
      for (let i = 0; i < keys.length - 1; i++) {
        const next = cur[keys[i]];
        if (!next || typeof next !== 'object') cur[keys[i]] = {};
        cur = cur[keys[i]] as Record<string, unknown>;
      }
      cur[keys[keys.length - 1]] = value;
    }
  });

  // Explicit per-path queue — verbatim to useSettingField, so each write lands
  // in the exact same debounced PUT bucket as a manual edit of that field.
  for (const { path, value } of writes) autoSaveManager.queueChange(path, value);
}

export interface MacroControl {
  value: number;
  onChange: (t: number) => void;
  onCommit: (t: number) => void;
}

export function useMacro(macro: MacroDef): MacroControl {
  const value = useSettingsStore((s) => macro.derive((p) => getByPath(s.settings, p)));
  const echoEnabled = useControlCenterUI((s) => s.echoPulseEnabled);

  const onChange = useCallback(
    (t: number) => {
      applyMacroWrites(macro.apply(t));
    },
    [macro],
  );

  const onCommit = useCallback(
    (t: number) => {
      applyMacroWrites(macro.apply(t));
      if (echoEnabled) emitEchoPulse({ origin: 'camera-center', strength: 0.6 });
    },
    [macro, echoEnabled],
  );

  return { value, onChange, onCommit };
}
