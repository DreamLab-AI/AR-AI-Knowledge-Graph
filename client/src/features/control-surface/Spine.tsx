/**
 * Spine — the unified control surface renderer.
 *
 * One component tree. Tailwind breakpoints handle mobile (<768px); no
 * separate <MobileSpine />. Per ADR-061 §Mobile rendering.
 *
 * Reads descriptors from the catalogue, filters by tier + visibleWhen +
 * search, groups by category, renders SettingRow per descriptor.
 */

import React, { useCallback, useMemo } from 'react';
import { useSpine } from './SpineProvider';
import { SettingRow } from './SettingRow';
import { SpineSearch, scoreDescriptor } from './SpineSearch';
import { useSettingsStore } from '@/store/settingsStore';
import type { Category, Setting } from './types';
import { pathToString } from './types';

interface SpineProps {
  descriptors: ReadonlyArray<Setting>;
  className?: string;
}

const CATEGORY_ORDER: ReadonlyArray<Category> = [
  'visual',
  'behaviour',
  'data',
  'team',
  'power',
  'operator',
];

const CATEGORY_TITLES: Record<Category, string> = {
  visual: 'Visual',
  behaviour: 'Behaviour',
  data: 'Data',
  team: 'Team',
  power: 'Power user',
  operator: 'Operator',
};

export function Spine({ descriptors, className }: SpineProps) {
  const { session, context, currentTier, setExpanded } = useSpine();
  const setByPath = useSettingsStore((s: any) => s.setByPath);
  const get = useSettingsStore((s: any) => s.get);

  // Tier filter (server-authoritative; this is UX hint).
  const tierFiltered = useMemo(
    () =>
      descriptors.filter(
        (d) => d.tier <= currentTier && (d.visibleWhen?.(context.state) ?? true)
      ),
    [descriptors, currentTier, context.state]
  );

  // Search filter / scoring. Empty query keeps all (score 1).
  const scored = useMemo(() => {
    return tierFiltered
      .map((d) => {
        const value = get?.(pathToString(d.path));
        const summary = (() => {
          try {
            return d.summary(value, context);
          } catch {
            return d.label;
          }
        })();
        const foldedLabels = (d.folds ?? [])
          .map((id) => descriptors.find((x) => x.id === id)?.label ?? '')
          .filter(Boolean);
        const score = scoreDescriptor(session.searchQuery, {
          label: d.label,
          summary,
          foldedLabels,
        });
        return { d, value, score };
      })
      .filter((x) => x.score > 0);
  }, [tierFiltered, descriptors, get, context, session.searchQuery]);

  const grouped = useMemo(() => {
    const m = new Map<Category, typeof scored>();
    for (const x of scored) {
      const arr = m.get(x.d.category) ?? [];
      arr.push(x);
      m.set(x.d.category, arr);
    }
    return m;
  }, [scored]);

  const onChangeFor = useCallback(
    (path: ReadonlyArray<string>) => (next: unknown) => {
      setByPath?.(pathToString(path), next);
    },
    [setByPath]
  );

  const topMatchId = scored[0]?.d.id ?? null;
  const handleEnterTopMatch = useCallback(() => {
    if (topMatchId) setExpanded(topMatchId);
  }, [topMatchId, setExpanded]);

  return (
    <div
      className={[
        'cs-spine',
        'flex flex-col w-full max-w-2xl mx-auto',
        'rounded-lg border border-slate-200/60 dark:border-slate-800/60',
        'bg-white/95 dark:bg-slate-950/95 shadow-lg',
        'overflow-hidden',
        className ?? '',
      ].join(' ')}
    >
      <SpineSearch onEnterTopMatch={handleEnterTopMatch} />

      <div className="flex-1 overflow-y-auto px-3 py-2 space-y-4 max-h-[calc(100vh-9rem)] sm:max-h-[80vh]">
        {CATEGORY_ORDER.map((cat) => {
          const items = grouped.get(cat);
          if (!items || items.length === 0) return null;
          const showDivider = cat === 'power' || cat === 'operator';
          return (
            <section key={cat} aria-labelledby={`cs-cat-${cat}`}>
              <h3
                id={`cs-cat-${cat}`}
                className={`mb-1 px-1 text-[11px] font-semibold uppercase tracking-wider ${
                  showDivider
                    ? 'text-amber-700 dark:text-amber-300 border-t border-dashed border-amber-300/40 dark:border-amber-700/40 pt-3 mt-2'
                    : 'text-slate-500 dark:text-slate-400'
                }`}
              >
                {CATEGORY_TITLES[cat]}
                {cat === 'power' && !context.isPowerUser && (
                  <span className="ml-2 font-normal italic">
                    (gated by power-user pubkey)
                  </span>
                )}
                {cat === 'operator' && !context.isOperator && (
                  <span className="ml-2 font-normal italic">
                    (gated by operator pubkey)
                  </span>
                )}
              </h3>
              <div className="space-y-1.5">
                {items.map(({ d, value }, i) => (
                  <SettingRow
                    key={d.id}
                    descriptor={d}
                    value={value}
                    expanded={session.expandedId === d.id}
                    annotate={session.annotate}
                    context={context}
                    highlight={
                      session.searchQuery.length > 0 && i === 0 && d.id === topMatchId
                    }
                    dim={session.searchQuery.length > 0 && i !== 0}
                    onToggle={() =>
                      setExpanded(session.expandedId === d.id ? null : d.id)
                    }
                    onChange={onChangeFor(d.path)}
                  />
                ))}
              </div>
            </section>
          );
        })}
        {scored.length === 0 && (
          <div className="px-2 py-8 text-center text-sm text-slate-500 dark:text-slate-400">
            {session.searchQuery
              ? `No settings match "${session.searchQuery}".`
              : 'No settings visible at this tier.'}
          </div>
        )}
      </div>
    </div>
  );
}
