/**
 * Generic editors used by descriptors. Authored once, reused 88+ times.
 * Each editor is a typed React.FC<EditorProps<T>>.
 */

import React, { useCallback } from 'react';
import type { EditorProps } from '../types';

// ─── Boolean toggle ─────────────────────────────────────────────────────

export function BooleanEditor({ value, onChange, descriptor }: EditorProps<boolean>) {
  return (
    <label className="flex items-center gap-2 text-sm">
      <input
        type="checkbox"
        checked={!!value}
        onChange={(e) => onChange(e.target.checked)}
        className="h-4 w-4 rounded border-slate-300"
        disabled={descriptor.readOnly}
      />
      <span className="text-slate-700 dark:text-slate-200">
        {descriptor.label}
      </span>
    </label>
  );
}

// ─── Number / slider ────────────────────────────────────────────────────

interface NumberEditorBounds {
  min?: number;
  max?: number;
  step?: number;
}

export function makeNumberEditor(bounds?: NumberEditorBounds) {
  const { min, max, step } = bounds ?? {};
  const E: React.FC<EditorProps<number>> = ({ value, onChange, descriptor }) => {
    const eff = typeof value === 'number' ? value : (min ?? 0);
    const handle = useCallback(
      (e: React.ChangeEvent<HTMLInputElement>) => {
        const n = Number(e.target.value);
        if (!Number.isFinite(n)) return;
        onChange(n);
      },
      [onChange]
    );
    const useSlider = min != null && max != null;
    return (
      <div className="flex flex-col gap-2">
        <div className="flex items-center gap-3">
          {useSlider && (
            <input
              type="range"
              value={eff}
              min={min}
              max={max}
              step={step ?? 0.01}
              onChange={handle}
              className="flex-1 accent-sky-500"
              disabled={descriptor.readOnly}
              aria-label={descriptor.label}
            />
          )}
          <input
            type="number"
            value={eff}
            min={min}
            max={max}
            step={step ?? 0.01}
            onChange={handle}
            className="w-24 rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 px-2 py-1 text-sm text-right tabular-nums"
            disabled={descriptor.readOnly}
            aria-label={`${descriptor.label} numeric input`}
          />
        </div>
        {min != null && max != null && (
          <div className="flex justify-between text-[10px] text-slate-500">
            <span>{min}</span>
            <span>{max}</span>
          </div>
        )}
      </div>
    );
  };
  return E;
}

export const NumberEditor = makeNumberEditor();

// ─── Color (hex) ────────────────────────────────────────────────────────

export function ColorEditor({ value, onChange, descriptor }: EditorProps<string>) {
  const v = typeof value === 'string' && value.startsWith('#') ? value : '#888888';
  return (
    <div className="flex items-center gap-2">
      <input
        type="color"
        value={v}
        onChange={(e) => onChange(e.target.value)}
        className="h-8 w-14 rounded border border-slate-300 cursor-pointer"
        disabled={descriptor.readOnly}
        aria-label={descriptor.label}
      />
      <input
        type="text"
        value={v}
        onChange={(e) => onChange(e.target.value)}
        className="w-28 rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 px-2 py-1 font-mono text-xs"
        disabled={descriptor.readOnly}
        aria-label={`${descriptor.label} hex code`}
      />
    </div>
  );
}

// ─── Enum / select ──────────────────────────────────────────────────────

interface EnumOption<T extends string> {
  value: T;
  label: string;
}

export function makeEnumEditor<T extends string>(options: ReadonlyArray<EnumOption<T>>) {
  const E: React.FC<EditorProps<T>> = ({ value, onChange, descriptor }) => (
    <select
      value={value}
      onChange={(e) => onChange(e.target.value as T)}
      className="w-full rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 px-2 py-1 text-sm"
      disabled={descriptor.readOnly}
      aria-label={descriptor.label}
    >
      {options.map((o) => (
        <option key={o.value} value={o.value}>
          {o.label}
        </option>
      ))}
    </select>
  );
  return E;
}

// ─── Read-only block (operator tier) ────────────────────────────────────

export function ReadOnlyEditor<T>({ value }: EditorProps<T>) {
  const text =
    typeof value === 'object' && value !== null
      ? JSON.stringify(value, null, 2)
      : String(value ?? '—');
  return (
    <pre className="overflow-x-auto rounded bg-slate-100 dark:bg-slate-900/80 p-2 font-mono text-[11px] text-slate-700 dark:text-slate-200">
      {text}
    </pre>
  );
}

// ─── Compound preset editor ─────────────────────────────────────────────

interface PresetSpec<T> {
  /** Preset name -> patch object that updates the children paths. */
  presets: Record<string, Partial<T>>;
  /** Determine current preset from value. Returns 'custom' if none match. */
  detectPreset: (v: T) => string;
}

export function makePresetEditor<T extends Record<string, any>>(spec: PresetSpec<T>) {
  const names = Object.keys(spec.presets);
  const E: React.FC<EditorProps<T>> = ({ value, onChange, descriptor }) => {
    const detected = spec.detectPreset(value);
    return (
      <div className="space-y-2">
        <div className="flex flex-wrap gap-1">
          {names.map((name) => (
            <button
              key={name}
              type="button"
              onClick={() => onChange({ ...value, ...spec.presets[name] })}
              className={`rounded-md border px-2.5 py-1 text-xs transition ${
                detected === name
                  ? 'border-sky-500 bg-sky-100 dark:bg-sky-900/40 text-sky-800 dark:text-sky-200'
                  : 'border-slate-300 dark:border-slate-700 hover:bg-slate-100 dark:hover:bg-slate-900'
              }`}
              disabled={descriptor.readOnly}
            >
              {name}
            </button>
          ))}
          {detected === 'custom' && (
            <span className="rounded-md border border-amber-400/60 bg-amber-100 dark:bg-amber-900/40 px-2.5 py-1 text-xs text-amber-800 dark:text-amber-200">
              custom
            </span>
          )}
        </div>
        <details className="text-xs text-slate-500 dark:text-slate-400">
          <summary className="cursor-pointer hover:text-slate-700 dark:hover:text-slate-200">
            Edit individual values
          </summary>
          <pre className="mt-2 rounded bg-slate-100 dark:bg-slate-900/80 p-2 font-mono text-[10px]">
            {JSON.stringify(value, null, 2)}
          </pre>
        </details>
      </div>
    );
  };
  return E;
}

// ─── Action button (for WIRE descriptors that trigger an HTTP call) ─────

interface ActionEditorProps<T> {
  buttonLabel: string;
  onClick: (ctx: { current: T }) => Promise<void> | void;
  /** Optional secondary disabled-state hint. */
  disabledHint?: string;
}

export function makeActionEditor<T>({
  buttonLabel,
  onClick,
  disabledHint,
}: ActionEditorProps<T>) {
  const E: React.FC<EditorProps<T>> = ({ value, descriptor }) => {
    const [busy, setBusy] = React.useState(false);
    const [msg, setMsg] = React.useState<string | null>(null);
    return (
      <div className="space-y-2">
        <button
          type="button"
          className="rounded-md bg-sky-600 px-3 py-1.5 text-sm font-medium text-white shadow-sm hover:bg-sky-500 disabled:opacity-50"
          disabled={busy || descriptor.readOnly}
          onClick={async () => {
            setBusy(true);
            setMsg(null);
            try {
              await onClick({ current: value });
              setMsg('done');
            } catch (e: any) {
              setMsg(e?.message ?? 'error');
            } finally {
              setBusy(false);
            }
          }}
        >
          {busy ? '…' : buttonLabel}
        </button>
        {msg && (
          <div className="text-xs text-slate-600 dark:text-slate-300">{msg}</div>
        )}
        {disabledHint && descriptor.readOnly && (
          <div className="text-xs italic text-slate-500">{disabledHint}</div>
        )}
      </div>
    );
  };
  return E;
}
