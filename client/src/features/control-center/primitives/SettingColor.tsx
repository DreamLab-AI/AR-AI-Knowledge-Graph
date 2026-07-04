/**
 * SettingColor — native <input type="color"> + hex readout. design-spec.md §2.
 */

import React, { useCallback } from 'react';
import type { RegistryField } from '../registry/types';
import { emitEchoPulse } from '../echo/echoPulseBus';

export interface SettingColorProps {
  field: RegistryField;
  testId: string;
  value: string;
  onChange: (value: string) => void;
  disabled?: boolean;
}

export const SettingColor: React.FC<SettingColorProps> = ({ field, testId, value, onChange, disabled }) => {
  const handleChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      onChange(e.target.value);
    },
    [onChange]
  );

  const handleCommit = useCallback(() => {
    emitEchoPulse({ origin: 'camera-center', strength: 0.3 });
  }, []);

  return (
    <div className="flex items-center gap-2">
      <input
        type="color"
        id={testId}
        data-testid={testId}
        value={value}
        onChange={handleChange}
        onBlur={handleCommit}
        disabled={disabled}
        aria-label={field.label}
        className="h-5 w-9 rounded border border-border cursor-pointer disabled:cursor-not-allowed disabled:opacity-50"
      />
      <span className="cc-value-readout">{value}</span>
    </div>
  );
};

SettingColor.displayName = 'SettingColor';

export default SettingColor;
