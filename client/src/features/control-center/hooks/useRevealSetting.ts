/**
 * useRevealSetting — bridges the command palette (and any `controlcenter:reveal`
 * emitter) to the shell. design-spec.md §3.4, §5.3, §6.3.
 *
 * On a reveal event {group, testid}: open the panel to that group → hydrate its
 * loadPaths → after the body renders, scroll the target control into view, flash
 * a 600ms highlight ring (WP1's `.cc-reveal-highlight`), and move focus to it.
 */

import { useEffect } from 'react';
import { GROUP_BY_ID } from '../registry/settingsRegistry';
import { useSettingsStore } from '../../../store/settingsStore';
import { useControlCenterUI } from '../state/useControlCenterUI';

export const REVEAL_EVENT = 'controlcenter:reveal';
/** Matches WP1's keyframe class in styles/control-center.css. */
const HIGHLIGHT_CLASS = 'cc-reveal-highlight';
const HIGHLIGHT_MS = 650;
/** Gives the panel + GroupSection a render tick before we locate the control. */
const RENDER_SETTLE_MS = 60;

export interface RevealDetail {
  group: string;
  testid: string;
}

export function useRevealSetting(): void {
  const openGroup = useControlCenterUI((s) => s.openGroup);

  useEffect(() => {
    const handler = (event: Event) => {
      const detail = (event as CustomEvent<RevealDetail>).detail;
      if (!detail?.testid) return;
      const { group, testid } = detail;

      openGroup(group);

      const g = GROUP_BY_ID[group];
      const hydrated = g
        ? useSettingsStore.getState().ensureLoaded(g.loadPaths)
        : Promise.resolve();

      Promise.resolve(hydrated)
        .catch(() => {
          /* fail-open: still attempt to reveal on its default value */
        })
        .finally(() => {
          requestAnimationFrame(() => {
            window.setTimeout(() => focusTarget(testid), RENDER_SETTLE_MS);
          });
        });
    };

    window.addEventListener(REVEAL_EVENT, handler);
    return () => window.removeEventListener(REVEAL_EVENT, handler);
  }, [openGroup]);
}

function focusTarget(testid: string): void {
  const el = document.querySelector<HTMLElement>(`[data-testid="${testid}"]`);
  if (!el) return;

  // scrollIntoView is absent under jsdom — guard so the reveal path stays testable.
  if (typeof el.scrollIntoView === 'function') {
    el.scrollIntoView({ block: 'center', behavior: 'smooth' });
  }

  const ring = (el.closest('.cc-row') as HTMLElement | null) ?? el;
  ring.classList.add(HIGHLIGHT_CLASS);
  window.setTimeout(() => ring.classList.remove(HIGHLIGHT_CLASS), HIGHLIGHT_MS);

  el.focus?.();
}
