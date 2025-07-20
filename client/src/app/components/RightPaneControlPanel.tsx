// client/src/app/components/RightPaneControlPanel.tsx
import React from 'react';
import { SettingsPanelRedesignOptimized } from '../../features/settings/components/panels/SettingsPanelRedesignOptimized';

interface RightPaneControlPanelProps {
  toggleLowerRightPaneDock: () => void;
  isLowerRightPaneDocked: boolean;
}

const RightPaneControlPanel: React.FC<RightPaneControlPanelProps> = ({ toggleLowerRightPaneDock, isLowerRightPaneDocked }) => {
  return (
    <div className="h-full flex flex-col bg-background">
      {/* The optimized settings panel now contains the auth section within its tabs */}
      <div className="flex-1 overflow-y-auto">
        <SettingsPanelRedesignOptimized
          toggleLowerRightPaneDock={toggleLowerRightPaneDock}
          isLowerRightPaneDocked={isLowerRightPaneDocked}
        />
      </div>
    </div>
  );
};

export default RightPaneControlPanel;