/**
 * SettingToggle — design-system Switch + label. design-spec.md §2.
 * Commit (the only meaningful transition for a boolean) fires the echo pulse.
 */

import React, { useCallback } from 'react';
import type { RegistryField } from '../registry/types';
import { Switch } from '../../design-system/components/Switch';
import { emitEchoPulse } from '../echo/echoPulseBus';

export interface SettingToggleProps {
  field: RegistryField;
  testId: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
}

export const SettingToggle: React.FC<SettingToggleProps> = ({ field, testId, checked, onChange, disabled }) => {
  const handleCheckedChange = useCallback(
    (next: boolean) => {
      onChange(next);
      emitEchoPulse({ origin: 'camera-center', strength: 0.4 });
    },
    [onChange]
  );

  return (
    <Switch
      id={testId}
      data-testid={testId}
      checked={checked}
      onCheckedChange={handleCheckedChange}
      disabled={disabled}
      aria-label={field.label}
    />
  );
};

SettingToggle.displayName = 'SettingToggle';

export default SettingToggle;
