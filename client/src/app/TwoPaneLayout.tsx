import React, { useState, useCallback, useRef, useEffect, CSSProperties } from 'react';
import GraphViewport from '../features/graph/components/GraphViewport';
import RightPaneControlPanel from './components/RightPaneControlPanel';
import ConversationPane from './components/ConversationPane';
import NarrativeGoldminePanel from './components/NarrativeGoldminePanel';
import { VoiceButton } from '../components/VoiceButton';
import { VoiceIndicator } from '../components/VoiceIndicator';
import { BrowserSupportWarning } from '../components/BrowserSupportWarning';

interface PaneDimensions {
  leftPaneWidth: number;
  rightPaneTopHeight: number;
  bottomRightUpperHeight: number;
}

const TwoPaneLayout: React.FC = () => {
  // Refs for DOM elements
  const rightPaneContainerRef = useRef<HTMLDivElement>(null);
  const rightPaneBottomContainerRef = useRef<HTMLDivElement>(null);
  const resizeObserverRef = useRef<ResizeObserver | null>(null);

  // Initialize leftPaneWidth to 80% of window width, or a fallback.
  const [dimensions, setDimensions] = useState<PaneDimensions>(() => {
    if (typeof window !== 'undefined') {
      return {
        leftPaneWidth: window.innerWidth * 0.8,
        rightPaneTopHeight: 200,
        bottomRightUpperHeight: 200
      };
    }
    return {
      leftPaneWidth: 600,
      rightPaneTopHeight: 200,
      bottomRightUpperHeight: 200
    };
  });

  const [isDraggingVertical, setIsDraggingVertical] = useState<boolean>(false);
  const [isRightPaneDocked, setIsRightPaneDocked] = useState<boolean>(false);
  
  // State for TOP horizontal splitter in right pane
  const [isDraggingHorizontalTop, setIsDraggingHorizontalTop] = useState<boolean>(false);
  
  // State for the BOTTOM horizontal splitter
  const [isDraggingHorizontalBottom, setIsDraggingHorizontalBottom] = useState<boolean>(false);
  const [isBottomPaneDocked, setIsBottomPaneDocked] = useState<boolean>(false);
  const [isLowerRightPaneDocked, setIsLowerRightPaneDocked] = useState<boolean>(false);

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
        // Ensure the new width is within reasonable bounds
        const minPaneWidth = 50;
        const maxPaneWidth = window.innerWidth - minPaneWidth - 10; // 10 for divider
        if (newWidth > minPaneWidth && newWidth < maxPaneWidth) {
          setDimensions(prev => ({ ...prev, leftPaneWidth: newWidth }));
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
      if (isDraggingHorizontalTop && rightPaneContainerRef.current) {
        const rightPaneRect = rightPaneContainerRef.current.getBoundingClientRect();
        const newTopPanelHeight = e.clientY - rightPaneRect.top;

        const minPanelHeight = 50;
        const dividerHeight = 10;

        // Ensure top panel doesn't get too small or too large
        if (newTopPanelHeight > minPanelHeight &&
            newTopPanelHeight < (rightPaneRect.height - (2 * minPanelHeight + 2 * dividerHeight))) {

            setDimensions(prev => {
              const remainingHeightForBottomTwo = rightPaneRect.height - newTopPanelHeight - dividerHeight;
              const heightForOneOfTheBottomTwo = (remainingHeightForBottomTwo - dividerHeight) / 2;
              
              return {
                ...prev,
                rightPaneTopHeight: newTopPanelHeight,
                bottomRightUpperHeight: heightForOneOfTheBottomTwo > minPanelHeight 
                  ? heightForOneOfTheBottomTwo 
                  : minPanelHeight
              };
            });
        }
      }
    },
    [isDraggingHorizontalTop]
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
      if (isDraggingHorizontalBottom && rightPaneBottomContainerRef.current) {
        const rect = rightPaneBottomContainerRef.current.getBoundingClientRect();
        const relativeNewHeight = e.clientY - rect.top;
        // Min 50 for upper, 60 for lower
        if (relativeNewHeight > 50 && relativeNewHeight < rect.height - 60) {
          setDimensions(prev => ({ ...prev, bottomRightUpperHeight: relativeNewHeight }));
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

  // ResizeObserver for responsive layout updates
  useEffect(() => {
    const updateLayout = () => {
      if (typeof window !== 'undefined') {
        if (!isDraggingVertical) {
          setDimensions(prev => ({
            ...prev,
            leftPaneWidth: isRightPaneDocked ? window.innerWidth : window.innerWidth * 0.8
          }));
        }

        // Calculate heights for right pane panels
        if (rightPaneContainerRef.current && !isDraggingHorizontalTop && !isDraggingHorizontalBottom) {
          const totalHeight = rightPaneContainerRef.current.clientHeight;
          const dividerHeight = 10;

          setDimensions(prev => {
            if (isLowerRightPaneDocked) {
              // When lower right pane is docked, top pane takes all available space
              return { ...prev, rightPaneTopHeight: totalHeight };
            } else if (isBottomPaneDocked) {
              // When bottom pane is docked, ConversationPane takes all available space
              const remainingHeight = totalHeight - prev.rightPaneTopHeight - dividerHeight;
              return { 
                ...prev, 
                bottomRightUpperHeight: remainingHeight > 50 ? remainingHeight : 50 
              };
            } else {
              // Normal three-panel split
              const panelHeight = (totalHeight - 2 * dividerHeight) / 3;
              return {
                ...prev,
                rightPaneTopHeight: panelHeight > 50 ? panelHeight : 50,
                bottomRightUpperHeight: panelHeight > 50 ? panelHeight : 50
              };
            }
          });
        }
      }
    };

    // Setup ResizeObserver
    if (rightPaneContainerRef.current && !resizeObserverRef.current) {
      resizeObserverRef.current = new ResizeObserver(updateLayout);
      resizeObserverRef.current.observe(rightPaneContainerRef.current);
    }

    updateLayout(); // Initial setup

    window.addEventListener('resize', updateLayout);
    return () => {
      window.removeEventListener('resize', updateLayout);
      if (resizeObserverRef.current) {
        resizeObserverRef.current.disconnect();
        resizeObserverRef.current = null;
      }
    };
  }, [isRightPaneDocked, isLowerRightPaneDocked, isBottomPaneDocked, isDraggingVertical, isDraggingHorizontalTop, isDraggingHorizontalBottom]);

  // Add and remove mouse move/up listeners on the window for dragging
  useEffect(() => {
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

  // Styles
  const containerStyle: CSSProperties = {
    display: 'flex',
    height: '100vh',
    overflow: 'hidden',
  };

  const leftPaneStyle: CSSProperties = {
    width: isRightPaneDocked ? '100%' : `${dimensions.leftPaneWidth}px`,
    minWidth: '50px',
    height: '100%',
    position: 'relative',
    transition: 'width 0.3s ease',
    borderRight: isRightPaneDocked ? 'none' : '1px solid #cccccc',
  };

  const dividerStyle: CSSProperties = {
    width: '10px',
    cursor: isRightPaneDocked ? 'default' : 'ew-resize',
    backgroundColor: '#cccccc',
    display: isRightPaneDocked ? 'none' : 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    userSelect: 'none',
  };

  const rightPaneContainerStyle: CSSProperties = {
    flexGrow: 1,
    display: isRightPaneDocked ? 'none' : 'flex',
    flexDirection: 'column',
    overflow: 'hidden',
    height: '100vh',
  };

  const rightPaneTopStyle: CSSProperties = {
    height: isLowerRightPaneDocked ? '100%' : `${dimensions.rightPaneTopHeight}px`,
    flexGrow: isLowerRightPaneDocked ? 1 : 0,
    minHeight: '50px',
    overflowY: 'auto',
    position: 'relative',
  };

  const horizontalTopDividerStyle: CSSProperties = {
    height: '10px',
    cursor: 'ns-resize',
    backgroundColor: '#b0b0b0',
    display: isLowerRightPaneDocked ? 'none' : 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    userSelect: 'none',
    borderTop: '1px solid #999999',
    borderBottom: '1px solid #999999',
    flexShrink: 0,
  };

  const rightPaneBottomContainerStyle: CSSProperties = {
    flexGrow: 1,
    minHeight: isLowerRightPaneDocked ? '0px' : '110px',
    display: isLowerRightPaneDocked ? 'none' : 'flex',
    flexDirection: 'column',
    overflow: 'hidden',
    backgroundColor: '#d0d0d0',
  };

  const bottomRightUpperStyle: CSSProperties = {
    height: isBottomPaneDocked ? 'auto' : `${dimensions.bottomRightUpperHeight}px`,
    flexGrow: isBottomPaneDocked ? 1 : 0,
    minHeight: '50px',
    padding: '0px',
    overflowY: 'hidden',
    position: 'relative',
  };

  const horizontalBottomDividerStyle: CSSProperties = {
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
    minHeight: isBottomPaneDocked ? '0px' : '50px',
    height: isBottomPaneDocked ? '0px' : 'auto',
    display: isBottomPaneDocked ? 'none' : 'flex',
    padding: '0px',
    overflowY: 'hidden',
    position: 'relative',
  };

  const dockButtonStyle: CSSProperties = {
    position: 'absolute',
    top: '10px',
    right: '10px',
    zIndex: 100,
    padding: '5px 10px',
    cursor: 'pointer',
    backgroundColor: 'rgba(255, 255, 255, 0.9)',
    border: '1px solid #ccc',
    borderRadius: '4px',
    fontSize: '14px',
    fontWeight: 'bold',
    boxShadow: '0 2px 4px rgba(0, 0, 0, 0.1)',
    transition: 'all 0.2s ease',
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
      <div ref={rightPaneContainerRef} style={rightPaneContainerStyle}>
        {!isRightPaneDocked && (
          <>
            <div style={rightPaneTopStyle}>
              <RightPaneControlPanel toggleLowerRightPaneDock={toggleLowerRightPaneDock} isLowerRightPaneDocked={isLowerRightPaneDocked} />
            </div>
            {!isLowerRightPaneDocked && (
              <>
                <div
                  style={horizontalTopDividerStyle}
                  onMouseDown={handleHorizontalTopMouseDown}
                  title="Drag to resize Control Panel / Lower Area"
                >
                  ══
                </div>
                <div ref={rightPaneBottomContainerRef} style={rightPaneBottomContainerStyle}>
                  <div style={bottomRightUpperStyle}>
                    <ConversationPane />
                    <button
                      onClick={toggleBottomPaneDock}
                      style={{
                        position: 'absolute',
                        bottom: '10px',
                        right: '10px',
                        zIndex: 100,
                        padding: '5px 10px',
                        cursor: 'pointer',
                        backgroundColor: 'rgba(255, 255, 255, 0.9)',
                        border: '1px solid #ccc',
                        borderRadius: '4px',
                        fontSize: '14px',
                        boxShadow: '0 2px 4px rgba(0, 0, 0, 0.1)',
                      }}
                      title={isBottomPaneDocked ? "Expand Lower Panel" : "Collapse Lower Panel"}
                    >
                      {isBottomPaneDocked ? '⬆' : '⬇'}
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
        {isRightPaneDocked ? '▶' : '◀'}
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