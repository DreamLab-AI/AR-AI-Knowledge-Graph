/**
 * SpineProvider — owns SpineSession aggregate.
 *
 * Reads URL on mount (per ADR-061 §State / store interaction) and writes
 * URL on state change. Computes currentTier from auth + user prefs; never
 * escalates beyond server-attached tier (multi-tenant invariant I06).
 */

import React, {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from 'react';

import type { SpineSession, SpineContext as SpineCtx, Tier } from './types';
import { parseSpineUrlParams, pushSpineSessionToUrl } from './deep-link';
import { useSettingsStore } from '@/store/settingsStore';

const INITIAL_SESSION: SpineSession = {
  expandedId: null,
  searchQuery: '',
  tierOverride: null,
  annotate: false,
};

interface SpineContextValue {
  session: SpineSession;
  context: SpineCtx;
  currentTier: Tier;
  setExpanded: (id: string | null) => void;
  setSearchQuery: (q: string) => void;
  setTierOverride: (t: Tier | null) => void;
  setAnnotate: (b: boolean) => void;
}

const Ctx = createContext<SpineContextValue | null>(null);

interface AuthState {
  pubkey?: string;
  isPowerUser: boolean;
  isOperator: boolean;
}

/**
 * TierResolver per DDD-control-surface §Domain services.
 * Defaults to 1; sticky advanced toggle → 2; PU pubkey → 3; operator pubkey → 4.
 * URL override cannot escalate above auth-attached effective tier.
 */
export function effectiveTier(auth: AuthState, advancedMode: boolean): Tier {
  if (auth.isOperator) return 4;
  if (auth.isPowerUser) return 3;
  if (advancedMode) return 2;
  return 1;
}

interface SpineProviderProps {
  children: React.ReactNode;
  auth?: AuthState;
}

export function SpineProvider({
  children,
  auth = { isPowerUser: false, isOperator: false },
}: SpineProviderProps) {
  const initialFromUrl = useMemo<SpineSession>(() => {
    if (typeof window === 'undefined') return INITIAL_SESSION;
    return { ...INITIAL_SESSION, ...parseSpineUrlParams(window.location.search) };
  }, []);

  const [session, setSession] = useState<SpineSession>(initialFromUrl);

  const advancedMode = useSettingsStore((s: any) =>
    s.get?.('user_preferences.advanced_mode') === true
  );

  const eff = effectiveTier(auth, advancedMode);

  // Cap any URL-supplied tier override at the auth-attached effective tier.
  const currentTier: Tier = (() => {
    if (session.tierOverride == null) return eff;
    return Math.min(session.tierOverride, eff) as Tier;
  })();

  // Push session changes back to URL.
  useEffect(() => {
    pushSpineSessionToUrl(session);
  }, [session]);

  const state = useSettingsStore((s: any) => s.settings);

  const context: SpineCtx = useMemo(
    () => ({
      state,
      pubkey: auth.pubkey,
      isPowerUser: auth.isPowerUser,
      isOperator: auth.isOperator,
    }),
    [state, auth.pubkey, auth.isPowerUser, auth.isOperator]
  );

  const setExpanded = useCallback(
    (id: string | null) => setSession((s) => ({ ...s, expandedId: id })),
    []
  );
  const setSearchQuery = useCallback(
    (q: string) => setSession((s) => ({ ...s, searchQuery: q })),
    []
  );
  const setTierOverride = useCallback(
    (t: Tier | null) => setSession((s) => ({ ...s, tierOverride: t })),
    []
  );
  const setAnnotate = useCallback(
    (b: boolean) => setSession((s) => ({ ...s, annotate: b })),
    []
  );

  const value: SpineContextValue = {
    session,
    context,
    currentTier,
    setExpanded,
    setSearchQuery,
    setTierOverride,
    setAnnotate,
  };

  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useSpine(): SpineContextValue {
  const v = useContext(Ctx);
  if (!v) {
    throw new Error('useSpine must be used inside <SpineProvider>');
  }
  return v;
}
