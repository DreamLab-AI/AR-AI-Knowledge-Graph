/**
 * useControlCenterUI — ephemeral UI state for the Control Center shell.
 * design-spec.md §4.3.
 *
 * Deliberately SEPARATE from the settings store: this is view state (which
 * surface is open, dock collapsed, echo-pulse flag) that must NEVER be
 * persisted to the backend, so it can't leak into the frozen 168-path
 * settings contract (§7 overrule 4). `echoPulseEnabled` is the Echo Pulse
 * feature flag — a UI concern, not a settings path — defaulting `true` and
 * forced `false` while the user prefers reduced motion.
 */

import { create } from 'zustand';

function prefersReducedMotion(): boolean {
  if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return false;
  try {
    return window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  } catch {
    return false;
  }
}

export interface ControlCenterUIState {
  /** Whether the slide-out SettingsPanel is open. */
  openPanel: boolean;
  /** Active rail entry: a semantic group id ('motion'…'system') or a bespoke
   *  panel id ('solid' | 'ontology'). Null when nothing is selected. */
  activeGroup: string | null;
  /** Dock collapsed to a single pill (Cmd/Ctrl+. hero mode). */
  dockCollapsed: boolean;
  /** Echo Pulse feature flag (forced false under prefers-reduced-motion). */
  echoPulseEnabled: boolean;

  /** Open the panel to a group/panel id (used by hotkeys, rail, reveal). */
  openGroup: (id: string) => void;
  /** Close the slide-out panel (keeps the last activeGroup for re-open). */
  closePanel: () => void;
  /** Toggle a group open/closed by id. */
  togglePanel: (id: string) => void;
  setActiveGroup: (id: string | null) => void;
  toggleDock: () => void;
  setDockCollapsed: (collapsed: boolean) => void;
  setEchoPulseEnabled: (enabled: boolean) => void;
}

export const useControlCenterUI = create<ControlCenterUIState>((set) => ({
  openPanel: false,
  activeGroup: null,
  dockCollapsed: false,
  echoPulseEnabled: !prefersReducedMotion(),

  openGroup: (id) => set({ openPanel: true, activeGroup: id }),
  closePanel: () => set({ openPanel: false }),
  togglePanel: (id) =>
    set((s) =>
      s.openPanel && s.activeGroup === id
        ? { openPanel: false }
        : { openPanel: true, activeGroup: id },
    ),
  setActiveGroup: (id) => set({ activeGroup: id }),
  toggleDock: () => set((s) => ({ dockCollapsed: !s.dockCollapsed })),
  setDockCollapsed: (collapsed) => set({ dockCollapsed: collapsed }),
  setEchoPulseEnabled: (enabled) => set({ echoPulseEnabled: enabled }),
}));

/**
 * Live reduced-motion binding: when the user's motion preference flips, the
 * echo-pulse flag follows it. Registered once at module load; fail-open so a
 * missing/broken matchMedia never throws during import (jsdom, older engines).
 */
if (typeof window !== 'undefined' && typeof window.matchMedia === 'function') {
  try {
    const mq = window.matchMedia('(prefers-reduced-motion: reduce)');
    const apply = () => useControlCenterUI.getState().setEchoPulseEnabled(!mq.matches);
    if (typeof mq.addEventListener === 'function') mq.addEventListener('change', apply);
    else if (typeof mq.addListener === 'function') mq.addListener(apply);
  } catch {
    /* no-op: reduced-motion binding is best-effort */
  }
}
