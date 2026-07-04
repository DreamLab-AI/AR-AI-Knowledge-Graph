/**
 * GlassPanel — the base glass surface used by every control-center overlay.
 * design-spec.md §5.1.
 */

import React from 'react';
import { cn } from '../../../utils/classNameUtils';
import '../styles/control-center.css';

export interface GlassPanelProps extends React.HTMLAttributes<HTMLDivElement> {
  /** Renders the accented ring variant (cc-glass--accent). */
  accent?: boolean;
  /** Applies the larger dock border radius instead of the default panel radius. */
  dockRadius?: boolean;
  /** Applies the 220ms panel slide transition class. */
  animated?: boolean;
}

export const GlassPanel = React.forwardRef<HTMLDivElement, GlassPanelProps>(
  ({ className, accent = false, dockRadius = false, animated = false, children, ...props }, ref) => {
    return (
      <div
        ref={ref}
        className={cn(
          'cc-glass',
          accent && 'cc-glass--accent',
          dockRadius && 'cc-glass--dock',
          animated && 'cc-panel-transition',
          className
        )}
        {...props}
      >
        {children}
      </div>
    );
  }
);

GlassPanel.displayName = 'GlassPanel';

export default GlassPanel;
