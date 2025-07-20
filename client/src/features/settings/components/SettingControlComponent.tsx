import React, { useState, useEffect, useCallback } from 'react';
import { UISettingDefinition } from '../config/settingsUIDefinition'; // Import the new definition type
import { Label } from '@/features/design-system/components/Label';
import { Slider } from '@/features/design-system/components/Slider';
import { Switch } from '@/features/design-system/components/Switch';
import { Input } from '@/features/design-system/components/Input';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/features/design-system/components/Select';
import { RadioGroup, RadioGroupItem } from '@/features/design-system/components/RadioGroup'; // Added RadioGroup imports
import { Button } from '@/features/design-system/components/Button';
import { Eye, EyeOff } from 'lucide-react';
import { HelpTooltip } from '../../help/components/HelpTooltip';
import { helpRegistry } from '../../help/HelpRegistry';

// Simple inline useDebounce hook
function useDebounce<T>(value: T, delay: number): T {
  const [debouncedValue, setDebouncedValue] = useState<T>(value);

  useEffect(() => {
    const handler = setTimeout(() => {
      setDebouncedValue(value);
    }, delay);

    return () => {
      clearTimeout(handler);
    };
  }, [value, delay]);

  return debouncedValue;
}

// Define props based on the plan
export interface SettingControlProps {
  path: string;
  settingDef: UISettingDefinition & { required?: boolean };
  value: any;
  onChange: (value: any) => void;
}

export const SettingControlComponent = React.memo(({ path, settingDef, value, onChange }: SettingControlProps) => {
  // State for debounced inputs
  const [inputValue, setInputValue] = useState(String(value ?? ''));
  const debouncedInputValue = useDebounce(inputValue, 300); // 300ms debounce
  const [showPassword, setShowPassword] = useState(false); // For password visibility toggle

  // Update internal state when the external value changes
  useEffect(() => {
    // Only update if the debounced value isn't the source of the change
    // This prevents loops but might need refinement depending on useDebounce implementation
    if (String(value) !== inputValue) {
       if (settingDef.type === 'rangeSlider' || settingDef.type === 'dualColorPicker') {
         // For array types, handle string conversion carefully if needed, or maybe skip input state?
         // For now, let's assume direct value prop usage for sliders/pickers is better for arrays.
       } else {
         setInputValue(String(value ?? ''));
       }
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [value, settingDef.type]); // Rerun if value or type changes

  // Effect to call onChange when debounced value changes
  useEffect(() => {
    if (settingDef.type === 'textInput' || settingDef.type === 'numberInput') {
      // Avoid calling onChange with the initial value or if it hasn't changed
      if (debouncedInputValue !== String(value ?? '')) {
        if (settingDef.type === 'numberInput') {
          const numValue = parseFloat(debouncedInputValue);
          if (!isNaN(numValue)) {
            onChange(numValue);
          }
        } else {
          onChange(debouncedInputValue);
        }
      }
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [debouncedInputValue, settingDef.type, onChange]); // Depend on debounced value

  // Handler for immediate input changes (updates local state)
  const handleInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    setInputValue(e.target.value);
  };

  // Render appropriate control based on settingDef.type
  const renderControl = () => {
    switch (settingDef.type) {
      case 'toggle':
        return (
          <Switch
            id={path}
            checked={Boolean(value)}
            onCheckedChange={onChange}
            label={settingDef.label}
          />
        );

      case 'slider': {
        // Defensive fallback if value is undefined or invalid
        const numericValue = typeof value === 'number' && !isNaN(value)
          ? value
          : (settingDef.min ?? 0);
        return (
          <div className="flex w-full items-center gap-3">
            <Slider
              id={path}
              value={[numericValue]}
              min={settingDef.min ?? 0}
              max={settingDef.max ?? 1}
              step={settingDef.step ?? 0.01}
              onValueChange={([val]) => onChange(val)}
              className="flex-1"
              label={settingDef.label}
              aria-valuemin={settingDef.min ?? 0}
              aria-valuemax={settingDef.max ?? 1}
              aria-valuenow={numericValue}
              aria-valuetext={`${numericValue.toFixed(settingDef.step && settingDef.step < 1 ? 2 : 0)}${settingDef.unit ? ` ${settingDef.unit}` : ''}`}
            />
            <span className="text-xs font-mono w-16 text-right tabular-nums">
              {numericValue.toFixed(settingDef.step && settingDef.step < 1 ? 2 : 0)}
              {settingDef.unit && <span className="ml-1">{settingDef.unit}</span>}
            </span>
          </div>
        );
      }

      case 'numberInput':
        // Always render an Input for 'numberInput' type.
        // If a slider is preferred, 'slider' type should be used in definition.
        return (
          <div className="flex items-center w-full">
            <Input
              id={path}
              type="number"
              value={inputValue}
              onChange={handleInputChange}
              min={settingDef.min}
              max={settingDef.max}
              step={settingDef.step ?? 1}
              className="h-8 flex-1 tabular-nums"
            />
            {settingDef.unit && <span className="text-xs text-muted-foreground pl-2">{settingDef.unit}</span>}
          </div>
        );

      case 'textInput':
        // Special handling for obscured fields like API keys
        const isSensitive = settingDef.label.toLowerCase().includes('key') ||
                           settingDef.label.toLowerCase().includes('secret') ||
                           settingDef.label.toLowerCase().includes('token');
        return (
          <div className="flex items-center w-full gap-2">
            <Input
              id={path}
              type={isSensitive && !showPassword ? "password" : "text"}
              value={inputValue} // Use local state for debouncing
              onChange={handleInputChange} // Update local state immediately
              className="h-8 flex-1" // Allow input to grow
              placeholder={isSensitive ? "Enter secure value" : "Enter value"}
            />
            {isSensitive && (
              <Button
                type="button"
                variant="ghost"
                size="icon"
                className="h-8 w-8"
                onClick={() => setShowPassword(!showPassword)}
                title={showPassword ? "Hide value" : "Show value"}
              >
                {showPassword ? <EyeOff className="h-4 w-4" aria-hidden="true" /> : <Eye className="h-4 w-4" aria-hidden="true" />}
              </Button>
            )}
            {settingDef.unit && <span className="text-xs text-muted-foreground">{settingDef.unit}</span>}
          </div>
        );

      case 'colorPicker':
        return (
          <div className="flex items-center gap-2">
            <Input
              id={path}
              type="color"
              value={String(value ?? '#000000')} // Ensure value is a string, default if null/undefined
              onChange={(e) => {
                // Ensure a valid hex color is always passed
                const newValue = e.target.value;
                if (/^#[0-9A-Fa-f]{6}$/i.test(newValue)) {
                  onChange(newValue);
                } else {
                  onChange('#000000'); // Fallback if somehow invalid from color input
                }
              }}
              className="h-8 w-10 p-0.5 border-border cursor-pointer"
              aria-label="Choose color"
            />
            <Input
              type="text"
              value={String(value ?? '')} // Reflect current value, allow empty for typing
              onChange={(e) => {
                const newValue = e.target.value;
                if (/^#[0-9A-Fa-f]{6}$/i.test(newValue)) {
                  onChange(newValue);
                } else if (newValue === '') {
                  // If user clears the input, set to a default to avoid sending empty string
                  // Or, you could choose not to call onChange, making the text input temporarily invalid
                  // For now, let's set a default to prevent server errors.
                  onChange('#000000'); // Default if cleared
                }
                // For other invalid inputs, we don't call onChange,
                // so the store isn't updated with an invalid partial hex.
                // The visual input will show the invalid text until corrected or blurred.
              }}
              onBlur={(e) => { // Ensure on blur, if invalid, it reverts or uses a default
                const currentValue = e.target.value;
                if (!/^#[0-9A-Fa-f]{6}$/i.test(currentValue)) {
                    // If current store value is valid, revert to it, else default
                    if (typeof value === 'string' && /^#[0-9A-Fa-f]{6}$/i.test(value)) {
                        onChange(value); // Revert to last known good value from store
                    } else {
                        onChange('#000000'); // Fallback to black
                    }
                }
              }}
              className="h-8 flex-1 font-mono text-xs"
              placeholder="#rrggbb"
            />
          </div>
        );

      case 'select':
        return (
          <Select
            value={String(value)} // Ensure value is string for Select
            onValueChange={(val) => onChange(val)} // Pass the string value back
          >
            <SelectTrigger id={path} className="h-8 w-full">
              <SelectValue placeholder={settingDef.label} />
            </SelectTrigger>
            <SelectContent>
              {settingDef.options?.map(opt => (
                <SelectItem key={String(opt.value)} value={String(opt.value)}>
                  {opt.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        );

      case 'radioGroup':
        return (
          <RadioGroup
            id={path}
            value={String(value)}
            onValueChange={(val) => onChange(val)}
            className="flex flex-row gap-4" // Arrange radio buttons horizontally
          >
            {settingDef.options?.map(opt => (
              <div key={String(opt.value)} className="flex items-center space-x-2">
                <RadioGroupItem value={String(opt.value)} id={`${path}-${opt.value}`} />
                <Label htmlFor={`${path}-${opt.value}`} className="text-sm font-normal">
                  {opt.label}
                </Label>
              </div>
            ))}
          </RadioGroup>
        );

      case 'rangeSlider': { // For [number, number] arrays
        const [minVal, maxVal] = Array.isArray(value) ? value : [settingDef.min ?? 0, settingDef.max ?? 1];
        const handleMinChange = (e: React.ChangeEvent<HTMLInputElement>) => {
          const newMin = parseFloat(e.target.value);
          if (!isNaN(newMin)) {
            onChange([newMin, maxVal]);
          }
        };
        const handleMaxChange = (e: React.ChangeEvent<HTMLInputElement>) => {
          const newMax = parseFloat(e.target.value);
          if (!isNaN(newMax)) {
            onChange([minVal, newMax]);
          }
        };
        return (
          <div className="flex flex-col gap-2">
            <div className="flex items-center gap-2">
              <Label htmlFor={`${path}-min`} className="text-xs w-10">Min:</Label>
              <Input id={`${path}-min`} type="number" value={minVal} onChange={handleMinChange} min={settingDef.min} max={maxVal} step={settingDef.step} className="h-8 flex-1" placeholder="Min" />
            </div>
            <div className="flex items-center gap-2">
              <Label htmlFor={`${path}-max`} className="text-xs w-10">Max:</Label>
              <Input id={`${path}-max`} type="number" value={maxVal} onChange={handleMaxChange} min={minVal} max={settingDef.max} step={settingDef.step} className="h-8 flex-1" placeholder="Max" />
            </div>
            {settingDef.unit && <span className="text-xs text-muted-foreground self-end">{settingDef.unit}</span>}
          </div>
        );
      }

      case 'dualColorPicker': { // For [string, string] color arrays
        const [color1 = '#ffffff', color2 = '#000000'] = Array.isArray(value) && value.length === 2 ? value : ['#ffffff', '#000000'];

        const createColorChangeHandler = (index: 0 | 1) => (e: React.ChangeEvent<HTMLInputElement>) => {
          const newColorValue = e.target.value;
          const currentColors = [color1, color2];

          if (/^#[0-9A-Fa-f]{6}$/i.test(newColorValue)) {
            currentColors[index] = newColorValue;
            onChange([...currentColors]);
          } else if (newColorValue === '') {
            currentColors[index] = '#000000'; // Default if cleared
            onChange([...currentColors]);
          }
          // For other invalid inputs, do not call onChange from text input
        };

        const createColorBlurHandler = (index: 0 | 1) => (e: React.ChangeEvent<HTMLInputElement>) => {
            const currentColors = [color1, color2];
            const blurredValue = e.target.value;
            if (!/^#[0-9A-Fa-f]{6}$/i.test(blurredValue)) {
                // Revert to original value for this specific color input if it was valid, else default
                const originalColorAtIndex = (Array.isArray(value) && value.length === 2 && typeof value[index] === 'string' && /^#[0-9A-Fa-f]{6}$/i.test(value[index])) ? value[index] : '#000000';
                currentColors[index] = originalColorAtIndex;
                onChange([...currentColors]);
            }
        };

        return (
          <div className="flex flex-col gap-2">
            <div className="flex items-center gap-2">
              <Label className="text-xs w-16">Start:</Label>
              <Input type="color" value={color1} onChange={createColorChangeHandler(0)} className="h-8 w-10 p-0.5 border-border cursor-pointer" title="Start Color" />
              <Input type="text" value={color1} onChange={createColorChangeHandler(0)} onBlur={createColorBlurHandler(0)} className="h-8 flex-1 font-mono text-xs" placeholder="#rrggbb" />
            </div>
            <div className="flex items-center gap-2">
              <Label className="text-xs w-16">End:</Label>
              <Input type="color" value={color2} onChange={createColorChangeHandler(1)} className="h-8 w-10 p-0.5 border-border cursor-pointer" title="End Color" />
              <Input type="text" value={color2} onChange={createColorChangeHandler(1)} onBlur={createColorBlurHandler(1)} className="h-8 flex-1 font-mono text-xs" placeholder="#rrggbb" />
            </div>
          </div>
        );
      }


      case 'buttonAction':
        return (
          <Button onClick={settingDef.action} size="sm" variant="outline">
            {settingDef.label} {/* Button text is the label */}
          </Button>
        );

      default:
        // Render value as string for unknown types
        return <span className="text-sm text-muted-foreground">{JSON.stringify(value)}</span>;
    }
  };

  // For button actions, the label is the button itself, so we don't need a separate label.
  if (settingDef.type === 'buttonAction') {
    return renderControl();
  }

  // Get help content from registry if available
  const helpId = `settings.${path.replace(/\./g, '.')}`;
  const helpContent = helpRegistry.getHelp(helpId);

  return (
    <div className="setting-control grid grid-cols-3 items-center gap-x-4 gap-y-2 py-3 border-b border-border/30 last:border-b-0 hover:bg-muted/20 transition-colors rounded-sm px-1 -mx-1">
      <div className="col-span-1 flex items-center"> {/* Label takes 1/3rd */}
        <HelpTooltip
          help={helpContent || settingDef.description || ''}
          showIndicator={!!settingDef.description || !!helpContent}
          side="top"
          align="start"
        >
          <Label htmlFor={path} className="text-sm cursor-help">
            {settingDef.label}
            {settingDef.required && <span className="text-destructive ml-1" aria-label="required">*</span>}
          </Label>
        </HelpTooltip>
      </div>
      <div className="col-span-2"> {/* Control takes 2/3rds */}
        {renderControl()}
      </div>
    </div>
  );
}, (prevProps, nextProps) => {
  // Custom comparison for better performance
  return (
    prevProps.path === nextProps.path &&
    prevProps.value === nextProps.value &&
    prevProps.settingDef === nextProps.settingDef &&
    prevProps.onChange === nextProps.onChange
  );
});

SettingControlComponent.displayName = 'SettingControlComponent';