/**
 * SettingSelect — design-system (Radix) Select. design-spec.md §2.
 * Radix renders an ARIA combobox/listbox rather than a native <select>;
 * data-testid lives on the trigger, which is the interactive, focusable
 * element CDP automation drives.
 */

import React, { useCallback } from 'react';
import type { RegistryField } from '../registry/types';
import {
  Select,
  SelectTrigger,
  SelectValue,
  SelectContent,
  SelectItem,
} from '../../design-system/components/Select';
import { emitEchoPulse } from '../echo/echoPulseBus';

export interface SettingSelectProps {
  field: RegistryField;
  testId: string;
  value: string;
  onChange: (value: string) => void;
  disabled?: boolean;
}

export const SettingSelect: React.FC<SettingSelectProps> = ({ field, testId, value, onChange, disabled }) => {
  const handleValueChange = useCallback(
    (next: string) => {
      onChange(next);
      emitEchoPulse({ origin: 'camera-center', strength: 0.3 });
    },
    [onChange]
  );

  return (
    <Select value={value} onValueChange={handleValueChange} disabled={disabled}>
      <SelectTrigger id={testId} data-testid={testId} aria-label={field.label} className="h-7 text-xs">
        <SelectValue placeholder={field.options?.[0]} />
      </SelectTrigger>
      <SelectContent>
        {field.options?.map((option) => (
          <SelectItem key={option} value={option}>
            {option}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
};

SettingSelect.displayName = 'SettingSelect';

export default SettingSelect;
