/**
 * SettingText — text input, commit on blur/Enter. design-spec.md §2.
 */

import React, { useCallback, useState, useEffect, useRef } from 'react';
import type { RegistryField } from '../registry/types';

export interface SettingTextProps {
  field: RegistryField;
  testId: string;
  value: string;
  onChange: (value: string) => void;
  disabled?: boolean;
}

export const SettingText: React.FC<SettingTextProps> = ({ field, testId, value, onChange, disabled }) => {
  const [draft, setDraft] = useState(value);

  // Sync the draft when the store value changes underneath us (macro co-drive,
  // palette reveal, external reset).
  useEffect(() => setDraft(value), [value]);

  // Commit reads the latest draft/value through refs rather than closing over
  // the render-time values. A handler that closes over a stale `draft` (or is
  // memoized without `draft` in its deps) is the classic commit-path bug: the
  // input looks accepted but blur/Enter commit an empty/stale string — or the
  // guard sees draft===value and never calls onChange, so the store never
  // receives the key. Refs make the setter fire the freshest text every time.
  const draftRef = useRef(draft);
  draftRef.current = draft;
  const valueRef = useRef(value);
  valueRef.current = value;

  const commit = useCallback(() => {
    const nextDraft = draftRef.current;
    if (nextDraft !== valueRef.current) onChange(nextDraft);
  }, [onChange]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLInputElement>) => {
      if (e.key === 'Enter') {
        commit();
        e.currentTarget.blur();
      }
    },
    [commit]
  );

  return (
    <input
      type="text"
      id={testId}
      data-testid={testId}
      value={draft}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={commit}
      onKeyDown={handleKeyDown}
      disabled={disabled}
      aria-label={field.label}
      className="w-full h-7 px-2 text-xs rounded border border-border bg-background/50 disabled:cursor-not-allowed disabled:opacity-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
    />
  );
};

SettingText.displayName = 'SettingText';

export default SettingText;
