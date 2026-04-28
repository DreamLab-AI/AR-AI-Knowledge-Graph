/**
 * NLPromptInline — per-row "describe in your own words" affordance.
 * Wires to /api/nl-query/translate per ADR-061 §LLM call envelope.
 *
 * On submit:
 *   1. Build envelope from the descriptor + current value.
 *   2. POST → server returns { action, path, value }.
 *   3. Apply optimistically through the existing onChange dispatch.
 *   4. Surface explanation to the user; on denial, show toast + revert.
 */

import React, { useCallback, useState } from 'react';
import type { Setting, SpineContext } from './types';
import { translateIntent, fetchExamples } from './llm/client';

interface NLPromptInlineProps<T> {
  descriptor: Setting<T>;
  value: T;
  context: SpineContext;
  onChange: (next: T) => void;
}

export function NLPromptInline<T>({
  descriptor,
  value,
  context,
  onChange,
}: NLPromptInlineProps<T>) {
  const [intent, setIntent] = useState('');
  const [busy, setBusy] = useState(false);
  const [explanation, setExplanation] = useState<string | null>(null);
  const [denial, setDenial] = useState<string | null>(null);
  const [examples, setExamples] = useState<string[]>(descriptor.llm?.examples ?? []);

  const onSubmit = useCallback(
    async (text: string) => {
      const trimmed = text.trim();
      if (!trimmed) return;
      setBusy(true);
      setDenial(null);
      setExplanation(null);
      try {
        const res = await translateIntent(trimmed, descriptor, value, context);
        if (res.action === 'set' && res.value !== undefined) {
          onChange(res.value as T);
          if (res.explanation) setExplanation(res.explanation);
          setIntent('');
        } else if (res.action === 'denied') {
          setDenial(res.reason ?? 'denied');
        } else if (res.action === 'noop') {
          setExplanation(res.explanation ?? 'No change.');
        }
      } finally {
        setBusy(false);
      }
    },
    [descriptor, value, context, onChange]
  );

  const onKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      void onSubmit(intent);
    }
  };

  const ensureExamples = useCallback(async () => {
    if (examples.length === 0) {
      const xs = await fetchExamples(descriptor);
      if (xs.length) setExamples(xs);
    }
  }, [descriptor, examples.length]);

  return (
    <div className="cs-nl-prompt mt-3 rounded-md border border-slate-300/40 dark:border-slate-700/60 bg-slate-50/60 dark:bg-slate-900/40 p-2">
      <label className="flex items-center gap-2 text-xs text-slate-600 dark:text-slate-300">
        <span aria-hidden>✨</span>
        <span>Describe in your own words</span>
      </label>
      <div className="mt-1 flex items-center gap-2">
        <input
          type="text"
          className="flex-1 rounded-md border border-slate-300/60 dark:border-slate-700 bg-white dark:bg-slate-950 px-2 py-1 text-sm text-slate-800 dark:text-slate-100 placeholder:text-slate-400 focus:outline-none focus:ring-1 focus:ring-sky-400"
          placeholder={examples[0] ?? 'tighter clusters'}
          value={intent}
          onChange={(e) => setIntent(e.target.value)}
          onFocus={() => void ensureExamples()}
          onKeyDown={onKeyDown}
          disabled={busy}
          aria-label={`Natural-language change for ${descriptor.label}`}
        />
        <button
          type="button"
          className="rounded-md bg-sky-600 px-3 py-1 text-xs font-medium text-white shadow-sm transition disabled:opacity-50 hover:bg-sky-500"
          disabled={busy || !intent.trim()}
          onClick={() => void onSubmit(intent)}
        >
          {busy ? '…' : 'Apply'}
        </button>
      </div>
      {examples.length > 1 && (
        <div className="mt-1 flex flex-wrap gap-1">
          {examples.slice(0, 4).map((ex) => (
            <button
              key={ex}
              type="button"
              className="rounded-full border border-slate-300/60 dark:border-slate-600 px-2 py-0.5 text-[11px] text-slate-600 dark:text-slate-300 transition hover:bg-slate-200 dark:hover:bg-slate-800"
              onClick={() => void onSubmit(ex)}
              disabled={busy}
            >
              {ex}
            </button>
          ))}
        </div>
      )}
      {explanation && (
        <div className="mt-2 rounded-md bg-emerald-50 dark:bg-emerald-950/40 border border-emerald-300/40 dark:border-emerald-800 px-2 py-1 text-xs text-emerald-800 dark:text-emerald-200">
          {explanation}
        </div>
      )}
      {denial && (
        <div className="mt-2 rounded-md bg-rose-50 dark:bg-rose-950/40 border border-rose-300/40 dark:border-rose-800 px-2 py-1 text-xs text-rose-800 dark:text-rose-200">
          Couldn't apply: {denial}
        </div>
      )}
    </div>
  );
}
