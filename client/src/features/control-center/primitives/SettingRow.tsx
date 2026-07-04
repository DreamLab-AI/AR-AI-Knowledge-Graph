/**
 * SettingRow — type-dispatch to the concrete control for one RegistryField.
 * design-spec.md §2, §5.2, §9.1.
 *
 * Row layout: label (11px) + description tooltip + power-user lock, control
 * dispatched by `field.type`. Honors `showWhen` against `localValues`.
 * data-testid lives on the control's underlying native element, computed
 * here per the §9.1 convention and passed down as a prop.
 */

import React from 'react';
import { Lock, Info } from 'lucide-react';
import type { RegistryField } from '../registry/types';
import { useSettingField } from '../hooks/useSettingField';
import { useSettingsStore } from '../../../store/settingsStore';
import { Tooltip, TooltipProvider } from '../../design-system/components/Tooltip';
import { SettingSlider } from './SettingSlider';
import { SettingToggle } from './SettingToggle';
import { SettingColor } from './SettingColor';
import { SettingSelect } from './SettingSelect';
import { SettingText } from './SettingText';
import { SettingAction } from './SettingAction';
import { NostrAuthControl } from './NostrAuthControl';
import { RendererInfo } from './RendererInfo';
import '../styles/control-center.css';

export interface SettingRowProps {
  field: RegistryField;
  groupId: string;
  /** Values for this group's transient localKey fields, keyed by localKey. */
  localValues?: Record<string, unknown>;
  onLocalChange?: (localKey: string, value: unknown) => void;
  disabled?: boolean;
}

export function testIdFor(field: RegistryField, groupId: string): string {
  return field.path ? `setting-${field.path}` : `setting-${groupId}.${field.key}`;
}

const NO_HEADER_TYPES = new Set(['action-button', 'nostr-button', 'readonly']);

export const SettingRow: React.FC<SettingRowProps> = ({ field, groupId, localValues, onLocalChange, disabled }) => {
  // Rules-of-hooks safe: `field` is stable for the lifetime of this row, so
  // calling useSettingField unconditionally (with '' when unused) never
  // changes the number/order of hooks across renders.
  const [storeValue, setStoreValue] = useSettingField<unknown>(field.path ?? '');
  // Power-user gating mirrors UnifiedSettingsTabContent.canWrite() exactly:
  // power-user-only fields are read-only until an authenticated power user
  // is present; local settings are otherwise always writable.
  const isPowerUser = useSettingsStore((s) => s.isPowerUser);

  if (field.showWhen) {
    const current = localValues?.[field.showWhen.localKey];
    if (current !== field.showWhen.equals) return null;
  }

  const isLocal = !!field.localKey;
  const value = isLocal ? localValues?.[field.localKey as string] : storeValue;
  const setValue = isLocal
    ? (v: unknown) => onLocalChange?.(field.localKey as string, v)
    : setStoreValue;

  const testId = testIdFor(field, groupId);
  const isWritable = !disabled && (!field.isPowerUserOnly || isPowerUser);

  const header = !NO_HEADER_TYPES.has(field.type) && (
    <div className="cc-row-header">
      <div className="cc-row-label-group">
        <label htmlFor={testId} className="cc-field-label">
          {field.label}
        </label>
        {field.description && (
          <TooltipProvider delayDuration={300}>
            <Tooltip content={field.description}>
              <span tabIndex={-1}>
                <Info size={10} className="text-muted-foreground" aria-hidden="true" />
              </span>
            </Tooltip>
          </TooltipProvider>
        )}
        {field.isPowerUserOnly && <Lock size={10} className="text-amber-500" aria-label="Power user only" />}
      </div>
    </div>
  );

  const renderControl = () => {
    switch (field.type) {
      case 'slider':
        return (
          <SettingSlider
            field={field}
            testId={testId}
            value={typeof value === 'number' ? value : Number(value) || 0}
            onChange={(v) => setValue(v)}
            disabled={disabled}
          />
        );
      case 'toggle':
        return (
          <SettingToggle
            field={field}
            testId={testId}
            checked={!!value}
            onChange={(v) => setValue(v)}
            disabled={disabled}
          />
        );
      case 'color':
        return (
          <SettingColor
            field={field}
            testId={testId}
            value={typeof value === 'string' ? value : '#ffffff'}
            onChange={(v) => setValue(v)}
            disabled={disabled}
          />
        );
      case 'select':
        return (
          <SettingSelect
            field={field}
            testId={testId}
            value={typeof value === 'string' ? value : field.options?.[0] ?? ''}
            onChange={(v) => setValue(v)}
            disabled={disabled}
          />
        );
      case 'text':
        return (
          <SettingText
            field={field}
            testId={testId}
            value={typeof value === 'string' ? value : ''}
            onChange={(v) => setValue(v)}
            disabled={disabled}
          />
        );
      case 'action-button':
        return <SettingAction field={field} testId={testId} localValues={localValues} disabled={disabled} />;
      case 'nostr-button':
        return <NostrAuthControl field={field} testId={testId} />;
      case 'readonly':
        return <RendererInfo field={field} testId={testId} />;
      default:
        return null;
    }
  };

  return (
    <div
      className="cc-row"
      style={{ opacity: isWritable ? 1 : 0.5, pointerEvents: isWritable ? 'auto' : 'none' }}
    >
      {header}
      {renderControl()}
    </div>
  );
};

SettingRow.displayName = 'SettingRow';

export default SettingRow;
