import React, { useState, useEffect } from 'react';
import { Panel, PanelGroup, PanelResizeHandle } from 'react-resizable-panels';
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
  const [isLowerRightPaneDocked, setIsLowerRightPaneDocked] = useState(false);
  const [isBottomPaneDocked, setIsBottomPaneDocked] = useState(false);
  // const [hasVoiceSupport, setHasVoiceSupport] = useState(true);

  // useEffect(() => {
  //   const support = AudioInputService.getBrowserSupport();
  //   const isSupported = support.getUserMedia && support.isHttps && support.audioContext && support.mediaRecorder;
  //   setHasVoiceSupport(isSupported);
  // }, []);

  const toggleRightPaneDock = () => {
    setIsRightPaneDocked(!isRightPaneDocked);
  };

  const toggleLowerRightPaneDock = () => {
    setIsLowerRightPaneDocked(!isLowerRightPaneDocked);
  };

  const toggleBottomPaneDock = () => {
    setIsBottomPaneDocked(!isBottomPaneDocked);
  };

  return (
    <div className="h-screen w-full overflow-hidden relative">
      <PanelGroup direction="horizontal" className="h-full">
        {/* Left pane - Graph Canvas */}
        <Panel
          defaultSize={80}
          minSize={20}
          className="relative h-full flex flex-col"
          style={{
            transition: isRightPaneDocked ? 'none' : 'width 0.3s ease',
            width: isRightPaneDocked ? '100%' : undefined,
          }}
        >
          <div className="w-full flex-1" style={{ minHeight: 0 }}>
            <GraphCanvas />
          </div>
        </Panel>

        {/* Vertical resizer */}
        {!isRightPaneDocked && (
          <PanelResizeHandle className="w-2 bg-border hover:bg-accent transition-colors cursor-ew-resize flex items-center justify-center">
            <div className="w-px h-6 bg-muted-foreground opacity-50" />
          </PanelResizeHandle>
        )}

        {/* Right pane container */}
        {!isRightPaneDocked && (
          <Panel defaultSize={20} minSize={15} className="flex flex-col">
            <PanelGroup direction="vertical">
              {/* Top panel - Control Panel */}
              <Panel
                defaultSize={isLowerRightPaneDocked ? 100 : 33.3}
                minSize={15}
                className="relative overflow-auto"
              >
                <RightPaneControlPanel
                  toggleLowerRightPaneDock={toggleLowerRightPaneDock}
                  isLowerRightPaneDocked={isLowerRightPaneDocked}
                />
              </Panel>

              {/* Only show the rest if lower right pane isn't docked */}
              {!isLowerRightPaneDocked && (
                <>
                  {/* Horizontal resizer */}
                  <PanelResizeHandle className="h-2 bg-border hover:bg-accent transition-colors cursor-ns-resize flex items-center justify-center">
                    <div className="h-px w-6 bg-muted-foreground opacity-50" />
                  </PanelResizeHandle>

                  {/* Middle panel - Conversation */}
                  <Panel
                    defaultSize={isBottomPaneDocked ? 66.7 : 33.3}
                    minSize={15}
                    className="relative overflow-hidden"
                  >
                    <ConversationPane />
                    <button
                      onClick={toggleBottomPaneDock}
                      className="absolute bottom-2 right-2 z-10 px-2 py-1 text-sm bg-background/90 border border-border rounded hover:bg-accent transition-colors"
                      title={isBottomPaneDocked ? 'Expand Lower Panel' : 'Collapse Lower Panel'}
                    >
                      {isBottomPaneDocked ? '⬆' : '⬇'}
                    </button>
                  </Panel>

                  {/* Bottom panel - Narrative Goldmine */}
                  {!isBottomPaneDocked && (
                    <>
                      <PanelResizeHandle className="h-2 bg-border hover:bg-accent transition-colors cursor-ns-resize flex items-center justify-center">
                        <div className="h-px w-6 bg-muted-foreground opacity-50" />
                      </PanelResizeHandle>

                      <Panel defaultSize={33.4} minSize={15} className="relative overflow-hidden">
                        <NarrativeGoldminePanel />
                      </Panel>
                    </>
                  )}
                </>
              )}
            </PanelGroup>
          </Panel>
        )}
      </PanelGroup>

      {/* Dock/Undock button */}
      <button
        onClick={toggleRightPaneDock}
        className="fixed top-4 right-4 z-50 w-10 h-10 flex items-center justify-center text-sm font-medium bg-background/90 border border-border rounded-md shadow-lg hover:bg-accent transition-colors pointer-events-auto"
        style={{ maxWidth: '40px', maxHeight: '40px' }}
        title={isRightPaneDocked ? 'Expand Right Pane' : 'Collapse Right Pane'}
      >
        {isRightPaneDocked ? '▶' : '◀'}
      </button>

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