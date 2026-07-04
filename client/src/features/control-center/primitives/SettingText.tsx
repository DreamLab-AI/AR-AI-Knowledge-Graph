/**
 * SettingText — text input, commit on blur/Enter. design-spec.md §2.
 */

import React, { useCallback, useState, useEffect } from 'react';
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

  useEffect(() => setDraft(value), [value]);

  const commit = useCallback(() => {
    if (draft !== value) onChange(draft);
  }, [draft, value, onChange]);

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
