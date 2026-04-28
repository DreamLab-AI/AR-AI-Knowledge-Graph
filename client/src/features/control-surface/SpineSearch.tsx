/**
 * SpineSearch — top-of-spine input. Fuzzy match against summary text + label
 * + folded-child labels (multi-tenant invariant: indexes labels only, no values).
 *
 * Cmd/Ctrl-F focuses; Enter expands top match.
 */

import React, { useCallback, useEffect, useRef } from 'react';
import { useSpine } from './SpineProvider';

interface SpineSearchProps {
  onEnterTopMatch?: () => void;
}

export function SpineSearch({ onEnterTopMatch }: SpineSearchProps) {
  const { session, setSearchQuery } = useSpine();
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const isMod = e.metaKey || e.ctrlKey;
      if (isMod && e.key.toLowerCase() === 'f') {
        // Don't fight the browser find on pages without the spine focused.
        const spineEl = document.querySelector('.cs-spine');
        if (spineEl && spineEl.contains(document.activeElement)) {
          e.preventDefault();
          inputRef.current?.focus();
          inputRef.current?.select();
        }
      }
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, []);

  const onKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLInputElement>) => {
      if (e.key === 'Enter') {
        e.preventDefault();
        onEnterTopMatch?.();
      } else if (e.key === 'Escape') {
        setSearchQuery('');
        (e.target as HTMLInputElement).blur();
      }
    },
    [onEnterTopMatch, setSearchQuery]
  );

  return (
    <div className="cs-search sticky top-0 z-10 px-3 py-2 backdrop-blur bg-white/85 dark:bg-slate-950/85 border-b border-slate-200/60 dark:border-slate-800/60">
      <input
        ref={inputRef}
        type="search"
        autoComplete="off"
        spellCheck={false}
        placeholder="Search settings… (⌘F)"
        value={session.searchQuery}
        onChange={(e) => setSearchQuery(e.target.value)}
        onKeyDown={onKeyDown}
        className="w-full rounded-md border border-slate-300/60 dark:border-slate-700 bg-white dark:bg-slate-900 px-3 py-1.5 text-sm text-slate-800 dark:text-slate-100 placeholder:text-slate-400 focus:outline-none focus:ring-2 focus:ring-sky-400"
        aria-label="Search settings"
      />
    </div>
  );
}

/**
 * Pure fuzzy-match scorer. Returns 0 (no match) or a positive score (higher = better).
 * Indexes label, summary text, and folded-child labels.
 */
export function scoreDescriptor(
  query: string,
  haystack: { label: string; summary: string; foldedLabels: string[] }
): number {
  const q = query.trim().toLowerCase();
  if (!q) return 1; // empty query treats every row as a (trivial) match.
  let score = 0;
  const lbl = haystack.label.toLowerCase();
  const sum = haystack.summary.toLowerCase();
  if (lbl === q) score += 100;
  else if (lbl.startsWith(q)) score += 60;
  else if (lbl.includes(q)) score += 30;
  if (sum.includes(q)) score += 20;
  for (const c of haystack.foldedLabels) {
    if (c.toLowerCase().includes(q)) score += 10;
  }
  return score;
}
