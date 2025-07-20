import React, { useState, useCallback, CSSProperties } from 'react';
import GraphViewport from '../features/graph/components/GraphViewport';
import RightPaneControlPanel from './components/RightPaneControlPanel';
// import MarkdownDisplayPanel from './components/MarkdownDisplayPanel'; // Remove this
import ConversationPane from './components/ConversationPane'; // Add this
import NarrativeGoldminePanel from './components/NarrativeGoldminePanel';
import { VoiceButton } from '../components/VoiceButton';
import { VoiceIndicator } from '../components/VoiceIndicator';
import { BrowserSupportWarning } from '../components/BrowserSupportWarning';

const TwoPaneLayout: React.FC = () => {
  // Initialize leftPaneWidth to 80% of window width, or a fallback.
  const [leftPaneWidth, setLeftPaneWidth] = useState<number>(() => {
    if (typeof window !== 'undefined') {
      return window.innerWidth * 0.8;
    }
    return 600; // Fallback for environments without window object during initial SSR/render
  });
  const [isDraggingVertical, setIsDraggingVertical] = useState<boolean>(false);
  const [isRightPaneDocked, setIsRightPaneDocked] = useState<boolean>(false);

  // State for TOP horizontal splitter in right pane (dividing Control Panel and the rest)
  // Initial heights will be set by useEffect
  const [rightPaneTopHeight, setRightPaneTopHeight] = useState<number>(200);
  const [isDraggingHorizontalTop, setIsDraggingHorizontalTop] = useState<boolean>(false);

  // State for the new BOTTOM horizontal splitter within the lower part of the right pane
  const [bottomRightUpperHeight, setBottomRightUpperHeight] = useState<number>(200);
  const [isDraggingHorizontalBottom, setIsDraggingHorizontalBottom] = useState<boolean>(false);
  const [isBottomPaneDocked, setIsBottomPaneDocked] = useState<boolean>(false);
  const [isLowerRightPaneDocked, setIsLowerRightPaneDocked] = useState<boolean>(false); // New state for docking middle and bottom panes

  const handleVerticalMouseDown = useCallback((e: React.MouseEvent) => {
    setIsDraggingVertical(true);
    e.preventDefault();
  }, []);

  const handleVerticalMouseUp = useCallback(() => {
    setIsDraggingVertical(false);
  }, []);

  const handleVerticalMouseMove = useCallback(
    (e: MouseEvent) => {
      if (isDraggingVertical && !isRightPaneDocked) {
        const newWidth = e.clientX;
        // Ensure the new width is within reasonable bounds (e.g., not too small or too large)
        const minPaneWidth = 50; // Minimum width for any pane
        const maxPaneWidth = window.innerWidth - minPaneWidth - 10; // 10 for divider
        if (newWidth > minPaneWidth && newWidth < maxPaneWidth) {
          setLeftPaneWidth(newWidth);
        }
      }
    },
    [isDraggingVertical, isRightPaneDocked]
  );

  // Event handlers for TOP horizontal splitter
  const handleHorizontalTopMouseDown = useCallback((e: React.MouseEvent) => {
    setIsDraggingHorizontalTop(true);
    e.preventDefault();
  }, []);

  const handleHorizontalTopMouseUp = useCallback(() => {
    setIsDraggingHorizontalTop(false);
  }, []);

  const handleHorizontalTopMouseMove = useCallback(
    (e: MouseEvent) => {
      if (isDraggingHorizontalTop) {
        const rightPaneContainer = document.getElementById('right-pane-container');
        if (rightPaneContainer) {
            const rightPaneRect = rightPaneContainer.getBoundingClientRect();
            const newTopPanelHeight = e.clientY - rightPaneRect.top;

            const minPanelHeight = 50;
            const dividerHeight = 10; // From horizontalTopDividerStyle and horizontalBottomDividerStyle

            // Ensure top panel doesn't get too small or too large, leaving space for two other panels and two dividers
            if (newTopPanelHeight > minPanelHeight &&
                newTopPanelHeight < (rightPaneRect.height - (2 * minPanelHeight + 2 * dividerHeight))) {

                setRightPaneTopHeight(newTopPanelHeight);

                // Recalculate height for the middle panel (ConversationPane)
                // so it and the bottom panel (NarrativeGoldminePanel) share the remaining space equally.
                const remainingHeightForBottomTwo = rightPaneRect.height - newTopPanelHeight - dividerHeight;
                const heightForOneOfTheBottomTwo = (remainingHeightForBottomTwo - dividerHeight) / 2;

                if (heightForOneOfTheBottomTwo > minPanelHeight) {
                    setBottomRightUpperHeight(heightForOneOfTheBottomTwo);
                } else {
                    setBottomRightUpperHeight(minPanelHeight);
                    // If ConversationPane is at minHeight, NarrativeGoldminePanel will take the rest due to flex-grow.
                }
            }
        }
      }
    },
    [isDraggingHorizontalTop] // setBottomRightUpperHeight should be added if it's not stable via dispatch
  );

  // Event handlers for BOTTOM horizontal splitter
  const handleHorizontalBottomMouseDown = useCallback((e: React.MouseEvent) => {
    setIsDraggingHorizontalBottom(true);
    e.preventDefault();
  }, []);

  const handleHorizontalBottomMouseUp = useCallback(() => {
    setIsDraggingHorizontalBottom(false);
  }, []);

  const handleHorizontalBottomMouseMove = useCallback(
    (e: MouseEvent) => {
      if (isDraggingHorizontalBottom) {
        const bottomRightContainer = document.getElementById('right-pane-bottom-container');
        if (bottomRightContainer) {
            const rect = bottomRightContainer.getBoundingClientRect();
            const relativeNewHeight = e.clientY - rect.top;
            // Min 50 for upper (ConversationPane), 60 for lower (NarrativeGoldminePanel + its potential minHeight + divider)
            if (relativeNewHeight > 50 && relativeNewHeight < rect.height - 60) {
                 setBottomRightUpperHeight(relativeNewHeight);
            }
        }
      }
    },
    [isDraggingHorizontalBottom]
  );

  const toggleRightPaneDock = () => {
    setIsRightPaneDocked(!isRightPaneDocked);
  };

  const toggleBottomPaneDock = () => {
    setIsBottomPaneDocked(!isBottomPaneDocked);
  };

  const toggleLowerRightPaneDock = () => {
    setIsLowerRightPaneDocked(!isLowerRightPaneDocked);
  };

  // Effect for setting initial pane widths and heights
  React.useEffect(() => {
    const updateLayout = () => {
      if (typeof window !== 'undefined') {
        if (!isDraggingVertical) {
          setLeftPaneWidth(isRightPaneDocked ? window.innerWidth : window.innerWidth * 0.8);
        }

        // Calculate heights for right pane panels
        const rightPaneContainer = document.getElementById('right-pane-container');
        if (rightPaneContainer && !isDraggingHorizontalTop && !isDraggingHorizontalBottom) {
          const totalHeight = rightPaneContainer.clientHeight;
          const dividerHeight = 10;

          if (isLowerRightPaneDocked) {
            // When lower right pane is docked, top pane takes all available space
            setRightPaneTopHeight(totalHeight);
          } else if (isBottomPaneDocked) {
            // When bottom pane is docked, ConversationPane takes all available space below the top panel
            const remainingHeight = totalHeight - rightPaneTopHeight - dividerHeight;
            setBottomRightUpperHeight(remainingHeight > 50 ? remainingHeight : 50);
          } else {
            // Normal three-panel split
            const panelHeight = (totalHeight - 2 * dividerHeight) / 3;
            setRightPaneTopHeight(panelHeight > 50 ? panelHeight : 50);
            setBottomRightUpperHeight(panelHeight > 50 ? panelHeight : 50);
          }
        }
      }
    };

    updateLayout(); // Initial setup

    window.addEventListener('resize', updateLayout);
    return () => window.removeEventListener('resize', updateLayout);
  }, [isRightPaneDocked, isDraggingVertical, isDraggingHorizontalTop, isDraggingHorizontalBottom]);


  // Add and remove mouse move/up listeners on the window for dragging
  React.useEffect(() => {
    const handleGlobalMouseMove = (e: MouseEvent) => {
      handleVerticalMouseMove(e);
      handleHorizontalTopMouseMove(e);
      handleHorizontalBottomMouseMove(e);
    };

    const handleGlobalMouseUp = () => {
      handleVerticalMouseUp();
      handleHorizontalTopMouseUp();
      handleHorizontalBottomMouseUp();
    };

    if (isDraggingVertical || isDraggingHorizontalTop || isDraggingHorizontalBottom) {
      window.addEventListener('mousemove', handleGlobalMouseMove);
      window.addEventListener('mouseup', handleGlobalMouseUp);
    } else {
      window.removeEventListener('mousemove', handleGlobalMouseMove);
      window.removeEventListener('mouseup', handleGlobalMouseUp);
    }
    return () => {
      window.removeEventListener('mousemove', handleGlobalMouseMove);
      window.removeEventListener('mouseup', handleGlobalMouseUp);
    };
  }, [
      isDraggingVertical, isDraggingHorizontalTop, isDraggingHorizontalBottom,
      handleVerticalMouseMove, handleHorizontalTopMouseMove, handleHorizontalBottomMouseMove,
      handleVerticalMouseUp, handleHorizontalTopMouseUp, handleHorizontalBottomMouseUp
    ]);

  const containerStyle: CSSProperties = {
    display: 'flex',
    height: '100vh', // Full viewport height
    overflow: 'hidden', // Prevent scrollbars on the container itself
  };

  const leftPaneStyle: CSSProperties = {
    width: isRightPaneDocked ? '100%' : `${leftPaneWidth}px`,
    minWidth: '50px', // Minimum width for the left pane
    // backgroundColor: '#f0f0f0', // Removed, canvas has its own
    // padding: '20px', // Removed, GraphViewport handles its own layout
    // overflow: 'auto', // Removed, GraphViewport handles its own overflow/scroll
    height: '100%', // Ensure left pane takes full height for the canvas
    position: 'relative', // For potential absolute positioned elements within GraphViewport
    transition: 'width 0.3s ease', // Smooth transition for docking
    borderRight: isRightPaneDocked ? 'none' : '1px solid #cccccc', // Visual delineation
  };

  const dividerStyle: CSSProperties = {
    width: '10px',
    cursor: isRightPaneDocked ? 'default' : 'ew-resize', // Change cursor when docked
    backgroundColor: '#cccccc',
    display: isRightPaneDocked ? 'none' : 'flex', // Hide divider when docked
    alignItems: 'center',
    justifyContent: 'center',
    userSelect: 'none',
  };

  const rightPaneContainerStyle: CSSProperties = { // Renamed for clarity
    flexGrow: 1,
    display: isRightPaneDocked ? 'none' : 'flex', // Use flex column for top/bottom sections
    flexDirection: 'column',
    overflow: 'hidden', // Containing div handles overflow
    height: '100vh', // Ensure it takes full viewport height
  };

  const rightPaneTopStyle: CSSProperties = {
    height: isLowerRightPaneDocked ? '100%' : `${rightPaneTopHeight}px`, // Take full height when lower is docked
    flexGrow: isLowerRightPaneDocked ? 1 : 0, // Allow it to grow when lower is docked
    minHeight: '50px',
    // backgroundColor: '#e0e0e0', // Removed, panel has its own
    // padding: '10px', // Removed, panel has its own
    overflowY: 'auto',   // Allow scrolling of panel content
    position: 'relative',
  };

  const horizontalTopDividerStyle: CSSProperties = {
    height: '10px',
    cursor: 'ns-resize',
    backgroundColor: '#b0b0b0',
    display: isLowerRightPaneDocked ? 'none' : 'flex', // Hide when lower is docked
    alignItems: 'center',
    justifyContent: 'center',
    userSelect: 'none',
    borderTop: '1px solid #999999',
    borderBottom: '1px solid #999999',
    flexShrink: 0,
  };

  const rightPaneBottomContainerStyle: CSSProperties = {
    flexGrow: 1,
    minHeight: isLowerRightPaneDocked ? '0px' : '110px', // Collapse when lower is docked
    display: isLowerRightPaneDocked ? 'none' : 'flex', // Hide when lower is docked
    flexDirection: 'column',
    overflow: 'hidden',
    backgroundColor: '#d0d0d0', // Base for this area
  };

  const bottomRightUpperStyle: CSSProperties = {
    height: isBottomPaneDocked ? 'auto' : `${bottomRightUpperHeight}px`, // Auto height when docked
    flexGrow: isBottomPaneDocked ? 1 : 0, // Take all available space when docked
    minHeight: '50px',
    // backgroundColor: '#d8d8d8', // Panel has its own bg
    padding: '0px', // Panel has its own padding
    overflowY: 'hidden', // Panel handles scroll
    position: 'relative',
  };

  const horizontalBottomDividerStyle: CSSProperties = { // New divider
    height: '10px',
    cursor: 'ns-resize',
    backgroundColor: '#a0a0a0',
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    userSelect: 'none',
    borderTop: '1px solid #888888',
    borderBottom: '1px solid #888888',
    flexShrink: 0,
  };

  const bottomRightLowerStyle: CSSProperties = {
    flexGrow: 1,
    minHeight: isBottomPaneDocked ? '0px' : '50px', // Collapse when docked
    height: isBottomPaneDocked ? '0px' : 'auto', // Collapse when docked
    display: isBottomPaneDocked ? 'none' : 'flex', // Hide when docked
    // backgroundColor: '#c8c8c8', // Panel has its own bg
    padding: '0px', // Panel has its own padding
    overflowY: 'hidden', // Panel handles scroll
    position: 'relative',
  };

  const dockButtonStyle: CSSProperties = {
    position: 'absolute',
    top: '10px',
    right: isRightPaneDocked ? '10px' : `${10 + (isRightPaneDocked ? 0 : 0)}px`, // Adjust button position
    zIndex: 100,
    padding: '5px 10px',
    cursor: 'pointer',
  };


  return (
    <div style={containerStyle}>
      <div style={leftPaneStyle}>
        <GraphViewport />
      </div>
      <div
        style={dividerStyle}
        onMouseDown={!isRightPaneDocked ? handleVerticalMouseDown : undefined}
        title={isRightPaneDocked ? "" : "Drag to resize"}
      >
        ||
      </div>
      <div id="right-pane-container" style={rightPaneContainerStyle}>
        {!isRightPaneDocked && (
          <>
            <div style={rightPaneTopStyle}>
              <RightPaneControlPanel toggleLowerRightPaneDock={toggleLowerRightPaneDock} isLowerRightPaneDocked={isLowerRightPaneDocked} />
            </div>
            {!isLowerRightPaneDocked && ( // Hide divider and lower container when docked
              <>
                <div
                  style={horizontalTopDividerStyle}
                  onMouseDown={handleHorizontalTopMouseDown}
                  title="Drag to resize Control Panel / Lower Area"
                >
                  ══
                </div>
                <div id="right-pane-bottom-container" style={rightPaneBottomContainerStyle}>
                  <div style={bottomRightUpperStyle}>
                    <ConversationPane />
                    {/* Dock button for ConversationPane */}
                    <button
                      onClick={toggleBottomPaneDock}
                      style={{
                        position: 'absolute',
                        bottom: '10px',
                        right: '10px',
                        zIndex: 100,
                        padding: '5px 10px',
                        cursor: 'pointer',
                      }}
                      title={isBottomPaneDocked ? "Expand Lower Panel" : "Collapse Lower Panel"}
                    >
                      {isBottomPaneDocked ? '^' : 'v'}
                    </button>
                  </div>
                  {!isBottomPaneDocked && (
                    <>
                      <div
                        style={horizontalBottomDividerStyle}
                        onMouseDown={handleHorizontalBottomMouseDown}
                        title="Drag to resize Markdown / Narrative Goldmine"
                      >
                        ══
                      </div>
                      <div style={bottomRightLowerStyle}>
                        <NarrativeGoldminePanel />
                      </div>
                    </>
                  )}
                </div>
              </>
            )}
          </>
        )}
      </div>
      <button onClick={toggleRightPaneDock} style={dockButtonStyle} title={isRightPaneDocked ? "Expand Right Pane" : "Collapse Right Pane"}>
        {isRightPaneDocked ? '>' : '<'}
      </button>

      {/* Browser Support Warning - Top positioned */}
      <div
        style={{
          position: 'fixed',
          top: '20px',
          left: '50%',
          transform: 'translateX(-50%)',
          zIndex: 10000,
          maxWidth: '600px',
          width: '90%',
          pointerEvents: 'auto'
        }}
      >
        <BrowserSupportWarning />
      </div>

      {/* Voice Interaction Components - Floating UI */}
      <div
        style={{
          position: 'fixed',
          bottom: '20px',
          left: '20px',
          zIndex: 9999,
          display: 'flex',
          flexDirection: 'column',
          gap: '12px',
          alignItems: 'flex-start',
          pointerEvents: 'auto'
        }}
        className="voice-components-container"
      >
        <div style={{
          display: 'flex',
          flexDirection: 'column',
          gap: '8px',
          alignItems: 'center'
        }}>
          <VoiceButton size="lg" variant="primary" />
          <div style={{
            fontSize: '10px',
            color: 'rgba(255, 255, 255, 0.8)',
            textAlign: 'center',
            fontWeight: '500',
            textShadow: '0 1px 2px rgba(0, 0, 0, 0.8)'
          }}>
            Voice
          </div>
        </div>
        <VoiceIndicator
          className="max-w-md"
          showTranscription={true}
          showStatus={true}
        />
      </div>
    </div>
  );
};

export default TwoPaneLayout;