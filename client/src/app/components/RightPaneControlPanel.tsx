// client/src/app/components/RightPaneControlPanel.tsx
import React from 'react';
import { ChevronDown } from 'lucide-react';
import { SettingsPanelRedesignOptimized } from '../../features/settings/components/panels/SettingsPanelRedesignOptimized';
import ConversationPane from './ConversationPane';
import NarrativeGoldminePanel from './NarrativeGoldminePanel';
import { ProgrammaticMonitorControl } from '../../features/swarm/components/ProgrammaticMonitorControl';
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from '@/features/design-system/components/Collapsible';

const RightPaneControlPanel: React.FC = () => {
  return (
    <div className="h-full flex flex-col overflow-y-auto bg-background">
      {/* Settings Panel - Always visible at the top */}
      <div className="flex-shrink-0">
        <SettingsPanelRedesignOptimized />
      </div>

      {/* Conversation Pane - Collapsible */}
      <Collapsible defaultOpen={true} className="flex-shrink-0 border-t">
        <CollapsibleTrigger className="flex w-full items-center justify-between p-4 hover:bg-muted/50 transition-colors">
          <h3 className="text-sm font-semibold">Conversation</h3>
          <ChevronDown className="h-4 w-4 transition-transform duration-200 data-[state=closed]:-rotate-90" />
        </CollapsibleTrigger>
        <CollapsibleContent className="overflow-hidden">
          <div className="max-h-96 overflow-y-auto">
            <ConversationPane />
          </div>
        </CollapsibleContent>
      </Collapsible>

      {/* Narrative Goldmine Panel - Collapsible */}
      <Collapsible defaultOpen={true} className="flex-shrink-0 border-t">
        <CollapsibleTrigger className="flex w-full items-center justify-between p-4 hover:bg-muted/50 transition-colors">
          <h3 className="text-sm font-semibold">Narrative Goldmine</h3>
          <ChevronDown className="h-4 w-4 transition-transform duration-200 data-[state=closed]:-rotate-90" />
        </CollapsibleTrigger>
        <CollapsibleContent className="overflow-hidden">
          <div className="max-h-96 overflow-y-auto">
            <NarrativeGoldminePanel />
          </div>
        </CollapsibleContent>
      </Collapsible>

      {/* Programmatic Monitor Control - Collapsible */}
      <Collapsible defaultOpen={false} className="flex-shrink-0 border-t">
        <CollapsibleTrigger className="flex w-full items-center justify-between p-4 hover:bg-muted/50 transition-colors">
          <h3 className="text-sm font-semibold">Swarm Monitor</h3>
          <ChevronDown className="h-4 w-4 transition-transform duration-200 data-[state=closed]:-rotate-90" />
        </CollapsibleTrigger>
        <CollapsibleContent className="overflow-hidden">
          <div className="p-2">
            <ProgrammaticMonitorControl />
          </div>
        </CollapsibleContent>
      </Collapsible>
    </div>
  );
};

export default RightPaneControlPanel;