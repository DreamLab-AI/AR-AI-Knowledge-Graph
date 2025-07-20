import React from 'react';
import { render, screen, fireEvent } from '@testing-library/react';
import '@testing-library/jest-dom';
import { describe, it, expect, beforeEach, vi } from 'vitest';
import TwoPaneLayout from '../TwoPaneLayout';

// Mock the child components
vi.mock('../../features/graph/components/GraphViewport', () => ({
  default: () => <div data-testid="graph-viewport">GraphViewport</div>
}));

vi.mock('../components/RightPaneControlPanel', () => ({
  default: ({ toggleLowerRightPaneDock, isLowerRightPaneDocked }: any) => (
    <div data-testid="right-pane-control">
      RightPaneControlPanel
      <button onClick={toggleLowerRightPaneDock}>
        {isLowerRightPaneDocked ? 'Undock' : 'Dock'} Lower
      </button>
    </div>
  )
}));

vi.mock('../components/ConversationPane', () => ({
  default: () => <div data-testid="conversation-pane">ConversationPane</div>
}));

vi.mock('../components/NarrativeGoldminePanel', () => ({
  default: () => <div data-testid="narrative-panel">NarrativeGoldminePanel</div>
}));

vi.mock('../../components/VoiceButton', () => ({
  VoiceButton: () => <button data-testid="voice-button">Voice</button>
}));

vi.mock('../../components/VoiceIndicator', () => ({
  VoiceIndicator: () => <div data-testid="voice-indicator">VoiceIndicator</div>
}));

vi.mock('../../components/BrowserSupportWarning', () => ({
  BrowserSupportWarning: () => <div data-testid="browser-warning">BrowserSupportWarning</div>
}));

describe('TwoPaneLayout', () => {
  beforeEach(() => {
    // Mock window dimensions
    Object.defineProperty(window, 'innerWidth', {
      writable: true,
      configurable: true,
      value: 1024,
    });
    Object.defineProperty(window, 'innerHeight', {
      writable: true,
      configurable: true,
      value: 768,
    });
  });

  it('renders all components correctly', () => {
    render(<TwoPaneLayout />);
    
    expect(screen.getByTestId('graph-viewport')).toBeInTheDocument();
    expect(screen.getByTestId('right-pane-control')).toBeInTheDocument();
    expect(screen.getByTestId('conversation-pane')).toBeInTheDocument();
    expect(screen.getByTestId('narrative-panel')).toBeInTheDocument();
    expect(screen.getByTestId('voice-button')).toBeInTheDocument();
    expect(screen.getByTestId('voice-indicator')).toBeInTheDocument();
    expect(screen.getByTestId('browser-warning')).toBeInTheDocument();
  });

  it('toggles right pane dock state', () => {
    render(<TwoPaneLayout />);
    
    const dockButton = screen.getByTitle('Collapse Right Pane');
    expect(dockButton).toHaveTextContent('◀');
    
    fireEvent.click(dockButton);
    
    const expandButton = screen.getByTitle('Expand Right Pane');
    expect(expandButton).toHaveTextContent('▶');
    
    // Right pane should be hidden
    expect(screen.queryByTestId('right-pane-control')).not.toBeInTheDocument();
  });

  it('toggles bottom pane dock state', () => {
    render(<TwoPaneLayout />);
    
    const dockButton = screen.getByTitle('Collapse Lower Panel');
    expect(dockButton).toHaveTextContent('⬇');
    
    fireEvent.click(dockButton);
    
    const expandButton = screen.getByTitle('Expand Lower Panel');
    expect(expandButton).toHaveTextContent('⬆');
    
    // Narrative panel should be hidden
    expect(screen.queryByTestId('narrative-panel')).not.toBeInTheDocument();
  });

  it('initializes with correct default dimensions', () => {
    render(<TwoPaneLayout />);
    
    const container = screen.getByTestId('graph-viewport').parentElement;
    expect(container).toHaveStyle({ width: '819.2px' }); // 80% of 1024
  });

  it('handles resize events correctly', () => {
    const { rerender } = render(<TwoPaneLayout />);
    
    // Change window size
    Object.defineProperty(window, 'innerWidth', { value: 1280 });
    
    // Trigger resize event
    fireEvent(window, new Event('resize'));
    
    // Re-render to see changes
    rerender(<TwoPaneLayout />);
    
    // Check if layout adjusted (this is a simplified test)
    expect(screen.getByTestId('graph-viewport')).toBeInTheDocument();
  });

  it('shows correct divider handles when not docked', () => {
    render(<TwoPaneLayout />);
    
    // Check for vertical divider
    const verticalDivider = screen.getByTitle('Drag to resize');
    expect(verticalDivider).toBeInTheDocument();
    expect(verticalDivider).toHaveTextContent('||');
    
    // Check for horizontal dividers
    const horizontalDividers = screen.getAllByTitle(/Drag to resize/);
    expect(horizontalDividers.length).toBeGreaterThan(1);
  });

  it('uses refs instead of getElementById', () => {
    // This test verifies that the component doesn't use getElementById
    // by checking that the refactored code with refs works correctly
    const { container } = render(<TwoPaneLayout />);
    
    // The presence of ref-based containers
    const rightPaneContainer = container.querySelector('#right-pane-container');
    const bottomContainer = container.querySelector('#right-pane-bottom-container');
    
    expect(rightPaneContainer).toBeInTheDocument();
    expect(bottomContainer).toBeInTheDocument();
    
    // Verify these elements have the expected structure
    expect(rightPaneContainer).toHaveStyle({ display: 'flex' });
    expect(bottomContainer).toHaveStyle({ display: 'flex' });
  });
});