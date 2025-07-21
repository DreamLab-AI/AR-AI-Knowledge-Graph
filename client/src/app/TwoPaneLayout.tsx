import React, { useState, useEffect, useRef } from 'react';
import { Panel, PanelGroup, PanelResizeHandle } from 'react-resizable-panels';
import { ChevronLeft, ChevronRight } from 'lucide-react';
import GraphCanvas from '../features/graph/components/GraphCanvas';
import RightPaneControlPanel from './components/RightPaneControlPanel';
import ConversationPane from './components/ConversationPane';
import NarrativeGoldminePanel from './components/NarrativeGoldminePanel';
// import { VoiceButton } from '../components/VoiceButton';
// import { VoiceIndicator } from '../components/VoiceIndicator';
// import { BrowserSupportWarning } from '../components/BrowserSupportWarning';
// import { AudioInputService } from '../services/AudioInputService';

const TwoPaneLayout: React.FC = () => {
  const [isRightPaneDocked, setIsRightPaneDocked] = useState(false);
  const rightPanelRef = useRef<any>(null);

  // Ensure right panel is expanded on mount
  useEffect(() => {
    if (rightPanelRef.current && rightPanelRef.current.expand) {
      rightPanelRef.current.expand();
    }
  }, []);
  // const [hasVoiceSupport, setHasVoiceSupport] = useState(true);

  // useEffect(() => {
  //   const support = AudioInputService.getBrowserSupport();
  //   const isSupported = support.getUserMedia && support.isHttps && support.audioContext && support.mediaRecorder;
  //   setHasVoiceSupport(isSupported);
  // }, []);

  const toggleRightPaneDock = () => {
    if (rightPanelRef.current) {
      if (isRightPaneDocked) {
        rightPanelRef.current.expand();
      } else {
        rightPanelRef.current.collapse();
      }
    }
  };

  return (
    <div className="h-screen w-screen overflow-hidden bg-background text-foreground">
      <PanelGroup direction="horizontal" style={{ height: '100%' }}>
        {/* Left pane - Graph Canvas */}
        <Panel
          defaultSize={80}
          minSize={20}
          className="h-full"
        >
          <GraphCanvas />
        </Panel>

        {/* Vertical resizer with integrated dock button */}
        <PanelResizeHandle className="w-2 bg-gray-200 dark:bg-gray-800 hover:bg-accent transition-colors cursor-ew-resize relative group">
          <div className="absolute inset-0 flex items-center justify-center">
            <div className="w-px h-6 bg-muted-foreground opacity-75" />
          </div>
          {/* Integrated dock button in the resize handle */}
          <button
            onClick={toggleRightPaneDock}
            className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-6 h-12 flex items-center justify-center bg-background border border-border rounded opacity-0 group-hover:opacity-100 transition-opacity hover:bg-accent"
            title={isRightPaneDocked ? 'Show Panel' : 'Hide Panel'}
          >
            {isRightPaneDocked ? (
              <ChevronRight className="w-4 h-4" />
            ) : (
              <ChevronLeft className="w-4 h-4" />
            )}
          </button>
        </PanelResizeHandle>

        {/* Right pane - Single panel with all content */}
        <Panel
          ref={rightPanelRef}
          defaultSize={20}
          minSize={15}
          collapsible
          collapsedSize={0}
          onCollapse={() => setIsRightPaneDocked(true)}
          onExpand={() => setIsRightPaneDocked(false)}
          className="bg-background"
        >
          <PanelGroup direction="vertical">
            {/* Top panel - Control Panel */}
            <Panel
              defaultSize={33.3}
              minSize={15}
              className="relative overflow-auto"
            >
              <RightPaneControlPanel />
            </Panel>

            {/* Horizontal resizer */}
            <PanelResizeHandle className="h-2 bg-gray-200 dark:bg-gray-800 hover:bg-accent transition-colors cursor-ns-resize flex items-center justify-center">
              <div className="h-px w-6 bg-muted-foreground opacity-75" />
            </PanelResizeHandle>

            {/* Middle panel - Conversation */}
            <Panel
              defaultSize={33.3}
              minSize={15}
              className="relative overflow-hidden"
            >
              <ConversationPane />
            </Panel>

            {/* Horizontal resizer */}
            <PanelResizeHandle className="h-2 bg-gray-200 dark:bg-gray-800 hover:bg-accent transition-colors cursor-ns-resize flex items-center justify-center">
              <div className="h-px w-6 bg-muted-foreground opacity-75" />
            </PanelResizeHandle>

            {/* Bottom panel - Narrative Goldmine */}
            <Panel
              defaultSize={33.4}
              minSize={15}
              className="relative overflow-hidden"
            >
              <NarrativeGoldminePanel />
            </Panel>
          </PanelGroup>
        </Panel>
      </PanelGroup>

      {/* Browser Support Warning - Only show when there's no voice support */}
      {/* {!hasVoiceSupport && (
        <div className="fixed bottom-20 left-4 z-40 max-w-sm pointer-events-auto">
          <BrowserSupportWarning className="shadow-lg" />
        </div>
      )} */}

      {/* Voice Interaction Components - Only show when browser supports it */}
      {/* {hasVoiceSupport && (
        <div className="fixed bottom-4 left-4 z-50 flex flex-col gap-1 items-start pointer-events-auto">
          <VoiceButton size="md" variant="primary" />
          <VoiceIndicator
            className="max-w-xs text-xs"
            showTranscription={true}
            showStatus={false}
          />
        </div>
      )} */}
    </div>
  );
};

export default TwoPaneLayout;