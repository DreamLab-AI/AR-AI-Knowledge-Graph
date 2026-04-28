/**
 * Deep-link grammar for the Spine.
 *
 * URL params (per ADR-061 §Deep-link grammar):
 *   ?expand=<descriptor.id>            — opens that row
 *   ?expand=<id>&tier=1|2|3|4|max      — visual tier override (cannot escalate)
 *   ?annotate=1                        — show audit decision chips + § refs
 *   ?q=<search>                        — populate search box
 *
 * Server-side authority remains the security boundary; tier override is UX.
 */

import type { SpineSession, Tier } from './types';

const TIER_VALUES: ReadonlyArray<Tier> = [1, 2, 3, 4];

export function parseSpineUrlParams(search: string): Partial<SpineSession> {
  const params = new URLSearchParams(search);
  const out: Partial<SpineSession> = {};

  const expand = params.get('expand');
  if (expand) out.expandedId = expand;

  const q = params.get('q');
  if (q) out.searchQuery = q;

  const annotate = params.get('annotate');
  if (annotate === '1' || annotate === 'true') out.annotate = true;

  const tierStr = params.get('tier');
  if (tierStr === 'max') {
    out.tierOverride = 4;
  } else if (tierStr) {
    const n = Number.parseInt(tierStr, 10);
    if (TIER_VALUES.includes(n as Tier)) {
      out.tierOverride = n as Tier;
    }
  }

  return out;
}

/**
 * Build URL params from session, preserving any non-spine params that were
 * already on the URL (most importantly `surface=spine` itself, which the
 * MainLayout reads to decide which surface to render).
 */
export function serialiseSpineSession(session: SpineSession): string {
  const existing =
    typeof window !== 'undefined'
      ? new URLSearchParams(window.location.search)
      : new URLSearchParams();

  // Drop spine-managed keys; we'll re-set them below.
  for (const k of ['expand', 'q', 'annotate', 'tier']) existing.delete(k);

  if (session.expandedId) existing.set('expand', session.expandedId);
  if (session.searchQuery) existing.set('q', session.searchQuery);
  if (session.annotate) existing.set('annotate', '1');
  if (session.tierOverride != null) existing.set('tier', String(session.tierOverride));
  const s = existing.toString();
  return s ? `?${s}` : '';
}

/**
 * Apply the session to window.location without bouncing through React Router.
 * No-op in non-browser environments (SSR / tests). Preserves any non-spine
 * query params (notably `surface=spine`) so the surface toggle in MainLayout
 * does not lose its value on first session-state push.
 */
export function pushSpineSessionToUrl(session: SpineSession): void {
  if (typeof window === 'undefined' || !window.history) return;
  const qs = serialiseSpineSession(session);
  const next = `${window.location.pathname}${qs}${window.location.hash}`;
  if (next === `${window.location.pathname}${window.location.search}${window.location.hash}`) {
    return;
  }
  window.history.replaceState(null, '', next);
}
