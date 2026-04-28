/**
 * SettingRow — one row of the Spine.
 *
 * Collapsed view: descriptor.summary(value, ctx) renders as a sentence.
 * Expanded view: descriptor.Editor + per-row NL prompt + audit annotation chips.
 */

import React, { useCallback, useState } from 'react';
import type { Setting, SpineContext } from './types';
import { NLPromptInline } from './NLPromptInline';
import { explainDescriptor } from './llm/client';

interface SettingRowProps<T> {
  descriptor: Setting<T>;
  value: T;
  expanded: boolean;
  annotate: boolean;
  context: SpineContext;
  highlight?: boolean;
  dim?: boolean;
  onToggle: () => void;
  onChange: (next: T) => void;
}

export function SettingRow<T>({
  descriptor,
  value,
  expanded,
  annotate,
  context,
  highlight = false,
  dim = false,
  onToggle,
  onChange,
}: SettingRowProps<T>) {
  const [explainOpen, setExplainOpen] = useState(false);
  const [explanation, setExplanation] = useState<string | null>(null);
  const [explainBusy, setExplainBusy] = useState(false);

  const summary = (() => {
    try {
      return descriptor.summary(value, context);
    } catch (e) {
      return descriptor.label;
    }
  })();

  const requestExplain = useCallback(async () => {
    if (explanation) {
      setExplainOpen((o) => !o);
      return;
    }
    setExplainBusy(true);
    try {
      const res = await explainDescriptor(descriptor, value);
      setExplanation(res.explanation);
      setExplainOpen(true);
    } finally {
      setExplainBusy(false);
    }
  }, [descriptor, value, explanation]);

  const Editor = descriptor.Editor;

  const rowClasses = [
    'cs-row',
    'group flex flex-col rounded-md border transition',
    expanded
      ? 'border-sky-400/60 bg-sky-50/30 dark:bg-sky-950/20'
      : 'border-slate-200/60 dark:border-slate-800/60 bg-white/80 dark:bg-slate-950/40 hover:border-slate-300 dark:hover:border-slate-700',
    highlight ? 'ring-1 ring-sky-400/60' : '',
    dim ? 'opacity-40' : '',
    descriptor.readOnly ? 'cursor-default' : 'cursor-pointer',
  ].join(' ');

  return (
    <div className={rowClasses} data-descriptor-id={descriptor.id}>
      <button
        type="button"
        className="flex w-full items-center gap-2 px-3 py-2 text-left min-h-[2.5rem] leading-tight"
        style={{ minHeight: '2.5rem' }}
        onClick={onToggle}
        aria-expanded={expanded}
      >
        <span
          className="flex-1 text-sm text-slate-800 dark:text-slate-100"
          style={{
            minHeight: '1.25rem',
            lineHeight: '1.25rem',
            minWidth: 0,
            overflow: 'hidden',
            textOverflow: 'ellipsis',
            whiteSpace: 'nowrap',
          }}
        >
          {summary}
        </span>
        {annotate && descriptor.decision && (
          <span
            title={`audit: ${descriptor.decision}${descriptor.ref ? ` ${descriptor.ref}` : ''}`}
            className={`rounded-full px-2 py-0.5 text-[10px] font-medium ${
              descriptor.decision === 'KEEP'
                ? 'bg-slate-200 text-slate-700'
                : descriptor.decision === 'MERGE'
                  ? 'bg-violet-200 text-violet-800'
                  : descriptor.decision === 'EXPOSE'
                    ? 'bg-amber-200 text-amber-800'
                    : 'bg-emerald-200 text-emerald-800'
            }`}
          >
            {descriptor.decision}
          </span>
        )}
        {descriptor.readOnly && (
          <span className="rounded-md bg-slate-200/70 dark:bg-slate-800 px-1.5 py-0.5 text-[10px] uppercase tracking-wide text-slate-600 dark:text-slate-300">
            read-only
          </span>
        )}
        <span aria-hidden className="text-xs text-slate-400">
          {expanded ? '▾' : '▸'}
        </span>
      </button>

      {expanded && (
        <div className="border-t border-slate-200/60 dark:border-slate-800/60 p-3 space-y-3">
          <div>
            <Editor
              value={value}
              onChange={onChange}
              context={context}
              descriptor={descriptor}
            />
          </div>

          {!descriptor.readOnly && descriptor.llm !== undefined && (
            <NLPromptInline
              descriptor={descriptor}
              value={value}
              context={context}
              onChange={onChange}
            />
          )}

          <div className="flex items-center gap-2 text-[11px] text-slate-500">
            <button
              type="button"
              className="underline-offset-2 hover:underline"
              onClick={(e) => {
                e.stopPropagation();
                void requestExplain();
              }}
              disabled={explainBusy}
            >
              what does this do?
            </button>
            {annotate && descriptor.ref && (
              <span className="font-mono">{descriptor.ref}</span>
            )}
            <span className="font-mono opacity-60">{descriptor.id}</span>
          </div>

          {explainOpen && explanation && (
            <div className="rounded-md bg-slate-100 dark:bg-slate-900/60 px-3 py-2 text-xs text-slate-700 dark:text-slate-200">
              {explanation}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
