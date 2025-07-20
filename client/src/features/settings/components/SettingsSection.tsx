import React, { useState } from 'react'; // Added React import
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/features/design-system/components/Collapsible';
import { Card, CardContent, CardHeader, CardTitle } from '@/features/design-system/components/Card';
import { ChevronDown, ChevronUp, Minimize, Maximize } from 'lucide-react';
import { Button } from '@/features/design-system/components/Button';
// Removed import for SettingsSectionProps from types
// Removed import for SettingsSubsection
import Draggable from 'react-draggable';
import { useControlPanelContext } from './control-panel-context';
import { UISettingDefinition } from '../config/settingsUIDefinition'; // Import the definition type
import { SettingControlComponent } from './SettingControlComponent'; // Import the control component
import { useSettingsStore } from '@/store/settingsStore'; // Adjust path if necessary

// Define props locally
interface SettingsSectionProps {
  id: string;
  title: string;
  subsectionSettings: Record<string, UISettingDefinition>;
}

export function SettingsSection({ id, title, subsectionSettings }: SettingsSectionProps) {
  const [isOpen, setIsOpen] = useState(true);
  const [isDetached, setIsDetached] = useState(false);
  const { advancedMode } = useControlPanelContext();
  const settingsStore = useSettingsStore.getState(); // Get store state once
  const updateSettings = useSettingsStore(state => state.updateSettings); // Get updateSettings function

  // Removed advanced prop check at the top level

  // Removed old subsection mapping logic

  const handleDetach = () => {
    setIsDetached(!isDetached);
  };

  const renderSettings = () => (
    <div className="space-y-4">
      {Object.entries(subsectionSettings).map(([settingKey, settingDef]) => {
        // Visibility check: Advanced
        if (settingDef.isAdvanced && !advancedMode) {
          return null;
        }

        // Visibility/Read-only check: Power User
        const isPowerUser = useSettingsStore.getState().isPowerUser;
        if (settingDef.isPowerUserOnly && !isPowerUser) {
          // Decide whether to hide or show as read-only. Hiding for now.
          // TODO: Implement read-only display if needed
          return null;
        }

        // Retrieve value and define onChange handler
        const value = settingsStore.get(settingDef.path);
        const handleChange = (newValue: any) => {
          // Use updateSettings instead of deprecated set method
          updateSettings((draft) => {
            const pathParts = settingDef.path.split('.');
            let current: any = draft;
            
            // Navigate to the parent of the setting
            for (let i = 0; i < pathParts.length - 1; i++) {
              const part = pathParts[i];
              if (!current[part]) {
                current[part] = {};
              }
              current = current[part];
            }
            
            // Update the final value
            current[pathParts[pathParts.length - 1]] = newValue;
          });
        };

        return (
          <SettingControlComponent
            key={settingKey}
            path={settingDef.path}
            settingDef={settingDef}
            value={value}
            onChange={handleChange}
          />
        );
      })}
    </div>
  );

  if (isDetached) {
    return (
      <DetachedSection
        title={title}
        onReattach={handleDetach}
        sectionId={id}
      >
        <div className="p-2"> {/* Removed extra space-y-4, handled by renderSettings */}
          {renderSettings()}
        </div>
      </DetachedSection>
    );
  }

  return (
    <Card className="settings-section bg-card border border-border"> {/* Added background and border */}
      <CardHeader className="py-2 px-4">
        <Collapsible open={isOpen} onOpenChange={setIsOpen}>
          <div className="flex items-center justify-between">
            <CollapsibleTrigger asChild>
              <Button variant="ghost" size="sm" className="h-8 p-0 hover:bg-muted/50"> {/* Added hover effect */}
                <CardTitle className="text-sm font-medium text-card-foreground">{title}</CardTitle>
                {isOpen ? <ChevronUp className="ml-2 h-4 w-4 text-muted-foreground" /> : <ChevronDown className="ml-2 h-4 w-4 text-muted-foreground" />}
              </Button>
            </CollapsibleTrigger>
            <Button
              variant="ghost"
              size="icon"
              className="h-6 w-6 text-muted-foreground hover:text-card-foreground hover:bg-muted/50" // Added hover effect
              onClick={handleDetach}
              title="Detach section"
            >
              <Maximize className="h-3 w-3" />
            </Button>
          </div>

          <CollapsibleContent>
            <CardContent className="p-4 pt-3"> {/* Adjusted padding */}
              {renderSettings()}
            </CardContent>
          </CollapsibleContent>
        </Collapsible>
      </CardHeader>
    </Card>
  );
}

// Detached floating section component (Keep as is, but ensure it uses the new renderSettings)
function DetachedSection({
  children,
  title,
  onReattach,
  sectionId
}: {
  children: React.ReactNode;
  title: string;
  onReattach: () => void;
  sectionId: string;
}) {
  const [position, setPosition] = useState({ x: 100, y: 100 });

  const handleDrag = (e: any, data: { x: number; y: number }) => {
    setPosition({ x: data.x, y: data.y });
  };

  // Ensure the parent element for bounds exists and covers the intended area
  // If bounds="parent" doesn't work as expected, might need a specific selector or DOM element reference.

  return (
    <Draggable
      handle=".drag-handle" // Use a specific handle for dragging
      position={position}
      onDrag={handleDrag}
      bounds="body" // Changed bounds to body to allow freer movement
    >
      <div
        className="detached-panel absolute z-[3000] min-w-[300px] bg-card rounded-lg shadow-lg border border-border" // Added background, rounded corners
        data-section-id={sectionId}
      >
        <div className="drag-handle flex items-center justify-between border-b border-border p-2 cursor-move bg-muted/50 rounded-t-lg"> {/* Added handle class, background */}
          <div className="text-sm font-medium text-card-foreground">
            {title}
          </div>
          <Button
            variant="ghost"
            size="icon"
            className="h-6 w-6 text-muted-foreground hover:text-card-foreground hover:bg-muted/50" // Added hover effect
            onClick={onReattach}
            title="Reattach section"
          >
            <Minimize className="h-3 w-3" />
          </Button>
        </div>
        <div className="p-4 max-h-[400px] overflow-y-auto custom-scrollbar"> {/* Added padding, max-height and scroll */}
          {children}
        </div>
      </div>
    </Draggable>
  );
}