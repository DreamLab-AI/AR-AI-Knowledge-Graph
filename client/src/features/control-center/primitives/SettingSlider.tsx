/**
 * SettingSlider — native <input type="range">, uncontrolled during an active
 * pointer drag. design-spec.md §4.1, §9.1.
 *
 * Seeded from the store-backed `value` prop; `onInput`-equivalent (React's
 * onChange for range inputs) calls `onChange` on every tick, which hits the
 * store + debounced autosave — same perf floor as the legacy panel. External
 * value changes (macro co-drive, command-palette reveal) sync into the DOM
 * only while the user isn't actively dragging, so the drag never fights
 * React. Commit (pointerup / keyup, not per-tick) fires the echo pulse.
 */

import React, { useCallback, useEffect, useRef } from 'react';
import type { RegistryField } from '../registry/types';
import { emitEchoPulse } from '../echo/echoPulseBus';
import { cn } from '../../../utils/classNameUtils';
import '../styles/control-center.css';

export interface SettingSliderProps {
  field: RegistryField;
  testId: string;
  value: number;
  onChange: (value: number) => void;
  disabled?: boolean;
}

function formatValue(value: number, step?: number): string {
  if (step !== undefined && step < 0.01) return value.toFixed(5);
  if (step !== undefined && step < 1) return value.toFixed(2);
  return value.toFixed(0);
}

export const SettingSlider: React.FC<SettingSliderProps> = ({ field, testId, value, onChange, disabled }) => {
  const inputRef = useRef<HTMLInputElement>(null);
  const isDraggingRef = useRef(false);
  const min = field.min ?? 0;
  const max = field.max ?? 100;
  const step = field.step ?? 0.1;

  // Sync external value changes into the DOM only while the user isn't
  // mid-interaction, so a macro co-drive or palette reveal can move the
  // thumb without fighting an in-progress drag.
  useEffect(() => {
    if (!isDraggingRef.current && inputRef.current) {
      inputRef.current.value = String(value);
    }
  }, [value]);

  const handleInput = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      onChange(Number(e.target.value));
    },
    [onChange]
  );

  const handleCommit = useCallback(() => {
    isDraggingRef.current = false;
    emitEchoPulse({ origin: 'camera-center', strength: 0.6 });
  }, []);

  const handlePointerDown = useCallback(() => {
    isDraggingRef.current = true;
  }, []);

  return (
    <div className="flex flex-col gap-1">
      <div className="flex items-center justify-between">
        <span className="cc-value-readout" data-testid={`${testId}-readout`}>
          {formatValue(value, field.step)}
        </span>
        <span className="cc-helper-text">
          {min}–{max}
        </span>
      </div>
      <input
        ref={inputRef}
        type="range"
        id={testId}
        data-testid={testId}
        defaultValue={value}
        min={min}
        max={max}
        step={step}
        disabled={disabled}
        aria-label={field.label}
        aria-valuemin={min}
        aria-valuemax={max}
        aria-valuenow={value}
        onChange={handleInput}
        onPointerDown={handlePointerDown}
        onPointerUp={handleCommit}
        onKeyUp={handleCommit}
        className={cn(
          'w-full h-1.5 rounded-full appearance-none bg-secondary accent-primary',
          'cursor-pointer disabled:cursor-not-allowed disabled:opacity-50',
          'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring'
        )}
      />
    </div>
  );
};

SettingSlider.displayName = 'SettingSlider';

export default SettingSlider;
