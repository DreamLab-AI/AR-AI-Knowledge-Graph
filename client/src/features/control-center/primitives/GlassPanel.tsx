/**
 * GlassPanel — the base glass surface used by every control-center overlay.
 * design-spec.md §5.1.
 *
 * The left-docked SettingsPanel (which hosts the Solid/Ontology pods) is
 * drag-resizable from its free RIGHT edge. Because that panel's file is owned
 * elsewhere, GlassPanel auto-enables the resize affordance for it — detected by
 * its stable `data-testid` — while every other glass surface stays unaffected.
 * Resize can also be forced on/off explicitly via the `resizable` prop.
 */

import React from 'react';
import { cn } from '../../../utils/classNameUtils';
import {
  useControlCenterUI,
  PANEL_MIN_WIDTH,
  PANEL_MAX_WIDTH,
} from '../state/useControlCenterUI';
import { useResizable } from './useResizable';
import '../styles/control-center.css';

/** The docked panel that owns the persisted, user-resizable width. */
const DOCK_PANEL_TESTID = 'settings-panel';

export interface GlassPanelProps extends React.HTMLAttributes<HTMLDivElement> {
  /** Renders the accented ring variant (cc-glass--accent). */
  accent?: boolean;
  /** Applies the larger dock border radius instead of the default panel radius. */
  dockRadius?: boolean;
  /** Applies the 220ms panel slide transition class. */
  animated?: boolean;
  /** Force the drag-resize edge handle on/off. Defaults to auto (on for the
   *  docked settings panel, off for every other glass surface). */
  resizable?: boolean;
  /** Edge the resize handle sits on. 'right' for a left-docked panel. */
  resizeSide?: 'left' | 'right';
  /** Resize bounds (px). Default to the shared PANEL_MIN/MAX_WIDTH. */
  minWidth?: number;
  maxWidth?: number;
}

export const GlassPanel = React.forwardRef<HTMLDivElement, GlassPanelProps>(
  (
    {
      className,
      accent = false,
      dockRadius = false,
      animated = false,
      resizable,
      resizeSide = 'right',
      minWidth = PANEL_MIN_WIDTH,
      maxWidth = PANEL_MAX_WIDTH,
      children,
      style,
      ...props
    },
    ref,
  ) => {
    const testid = (props as { 'data-testid'?: string })['data-testid'];
    const enableResize = resizable ?? testid === DOCK_PANEL_TESTID;

    const panelWidth = useControlCenterUI((s) => s.panelWidth);
    const setPanelWidth = useControlCenterUI((s) => s.setPanelWidth);

    const { dragging, handleProps } = useResizable({
      enabled: enableResize,
      width: panelWidth,
      onResize: setPanelWidth,
      side: resizeSide,
      min: minWidth,
      max: maxWidth,
      label: 'Resize panel width',
    });

    // Inline width overrides the panel's Tailwind width class (e.g. w-[380px]);
    // merge so the caller's transform/opacity/pointer-events survive.
    const mergedStyle: React.CSSProperties | undefined = enableResize
      ? { ...style, width: panelWidth }
      : style;

    return (
      <div
        ref={ref}
        className={cn(
          'cc-glass',
          accent && 'cc-glass--accent',
          dockRadius && 'cc-glass--dock',
          animated && 'cc-panel-transition',
          className,
        )}
        style={mergedStyle}
        data-resizing={enableResize && dragging ? 'true' : undefined}
        {...props}
      >
        {enableResize && (
          <div
            {...handleProps}
            data-testid="panel-resize-handle"
            data-dragging={dragging}
            title="Drag to resize (or use arrow keys)"
            className={cn(
              'absolute top-0 bottom-0 z-50 flex w-2 items-center justify-center',
              'cursor-col-resize touch-none group',
              'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring',
              resizeSide === 'right' ? 'right-0' : 'left-0',
            )}
          >
            <span
              aria-hidden="true"
              className={cn(
                'h-10 w-0.5 rounded-full transition-colors',
                dragging ? 'bg-primary' : 'bg-border/70 group-hover:bg-primary/70',
              )}
            />
          </div>
        )}
        {children}
      </div>
    );
  },
);

GlassPanel.displayName = 'GlassPanel';

export default GlassPanel;
