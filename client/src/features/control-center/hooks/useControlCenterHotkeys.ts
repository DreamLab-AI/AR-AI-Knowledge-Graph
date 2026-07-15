/**
 * useControlCenterHotkeys — the keyboard map. design-spec.md §6.2.
 *
 * | Key            | Action                                                    |
 * |----------------|-----------------------------------------------------------|
 * | Cmd/Ctrl+K     | (owned by CommandPalette — NOT bound here, no double-bind)|
 * | 1–8            | open SettingsPanel to that semantic group                 |
 * | Cmd/Ctrl+.     | toggle dock (hero mode)                                   |
 * | Esc            | close the open SettingsPanel                              |
 * | ?              | open help                                                 |
 * | ←/→ ↑/↓        | (native to the focused dial/slider — not intercepted)     |
 *
 * Digit + '?' hotkeys are suppressed while a text field / CommandInput is focused.
 */

import { useEffect } from 'react';
import { REGISTRY } from '../registry/settingsRegistry';
import { useControlCenterUI } from '../state/useControlCenterUI';

function isTextEntry(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  const tag = target.tagName;
  if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return true;
  return target.isContentEditable;
}

export function useControlCenterHotkeys(): void {
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const ui = useControlCenterUI.getState();

      // Cmd/Ctrl+. → dock hero-mode toggle.
      if ((e.metaKey || e.ctrlKey) && e.key === '.') {
        e.preventDefault();
        ui.toggleDock();
        return;
      }

      // Cmd/Ctrl+K is the CommandPalette's — never double-bind. Ignore any other
      // modified chord so app/OS shortcuts pass through untouched.
      if (e.metaKey || e.ctrlKey || e.altKey) return;

      // Esc → close the top-most Control Center surface (the panel). The palette
      // owns its own Esc; the dock has an explicit toggle, so Esc is scoped to
      // the panel and left inert otherwise (canvas/global Esc handlers survive).
      if (e.key === 'Escape') {
        if (ui.openPanel) {
          // A Radix Select dropdown (or any popper-positioned overlay) open inside
          // the panel owns Escape first — it should close only itself, not the whole
          // panel. The design-system Select renders position="popper" content, so an
          // open dropdown is present as a [data-radix-popper-content-wrapper] with a
          // role="listbox"/data-state="open" body. If one is mounted, bail and let
          // Radix's own Escape handler dismiss the dropdown. See defect-2.
          if (
            typeof document !== 'undefined' &&
            document.querySelector('[data-radix-popper-content-wrapper], [role="listbox"]')
          ) {
            return;
          }
          e.preventDefault();
          ui.closePanel();
        }
        return;
      }

      // Everything below is suppressed while typing.
      if (isTextEntry(e.target)) return;

      // 1–9 → open the matching semantic group (realises the old dead buttonKey badges).
      if (e.key >= '1' && e.key <= '9') {
        const group = REGISTRY[Number(e.key) - 1];
        if (group) {
          e.preventDefault();
          ui.openGroup(group.id);
        }
        return;
      }

      // ? → help (broadcast; a help surface may subscribe).
      if (e.key === '?') {
        e.preventDefault();
        window.dispatchEvent(new CustomEvent('controlcenter:help'));
      }
    };

    // Capture phase: Radix's document-level Escape handler removes the open
    // popper synchronously before bubble-phase listeners run, so a bubble
    // listener's open-popover probe always misses and Escape collapses both
    // the dropdown and the panel. Capturing first sees the popper while it is
    // still mounted and bails, leaving the dropdown dismissal to Radix.
    window.addEventListener('keydown', handler, true);
    return () => window.removeEventListener('keydown', handler, true);
  }, []);
}
