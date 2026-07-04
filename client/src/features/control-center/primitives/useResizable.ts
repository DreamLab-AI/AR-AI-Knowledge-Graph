/**
 * useResizable — pointer-drag + keyboard resize logic for a docked GlassPanel.
 * design-spec.md §6.3 (non-modal dock surfaces stay interactive).
 *
 * Owns the interaction only; the width itself is controlled by the caller
 * (persisted in useControlCenterUI). Implements the WAI-ARIA window-splitter
 * pattern: a focusable `role="separator"` with aria-valuemin/max/now that
 * supports both pointer drag and arrow/Home/End keyboard resize.
 *
 * The panel is left-docked (its free edge is the RIGHT edge), so `side`
 * defaults to 'right': dragging right widens, ArrowRight/ArrowUp widens.
 */

import React, { useCallback, useRef, useState } from 'react';

export interface UseResizableOptions {
  /** Current width (px), controlled by the caller. */
  width: number;
  /** Commit a new width (the caller clamps + persists). */
  onResize: (width: number) => void;
  /** When false the handle is inert (tabIndex -1, handlers no-op). */
  enabled?: boolean;
  /** Which edge the handle sits on. 'right' for a left-docked panel. */
  side?: 'left' | 'right';
  min?: number;
  max?: number;
  /** Keyboard step in px. */
  step?: number;
  /** Accessible name for the separator. */
  label?: string;
}

/** Props to spread onto the handle element (a div). */
export interface ResizableHandleProps {
  role: 'separator';
  'aria-orientation': 'vertical';
  'aria-label': string;
  'aria-valuemin': number;
  'aria-valuemax': number;
  'aria-valuenow': number;
  tabIndex: number;
  onPointerDown: (e: React.PointerEvent<HTMLDivElement>) => void;
  onPointerMove: (e: React.PointerEvent<HTMLDivElement>) => void;
  onPointerUp: (e: React.PointerEvent<HTMLDivElement>) => void;
  onPointerCancel: (e: React.PointerEvent<HTMLDivElement>) => void;
  onKeyDown: (e: React.KeyboardEvent<HTMLDivElement>) => void;
}

export interface UseResizableResult {
  dragging: boolean;
  handleProps: ResizableHandleProps;
}

export function useResizable({
  width,
  onResize,
  enabled = true,
  side = 'right',
  min = 0,
  max = Number.POSITIVE_INFINITY,
  step = 16,
  label = 'Resize panel',
}: UseResizableOptions): UseResizableResult {
  const [dragging, setDragging] = useState(false);
  const dragRef = useRef<{ startX: number; startWidth: number } | null>(null);

  const clamp = useCallback(
    (w: number) => Math.min(max, Math.max(min, Math.round(w))),
    [max, min],
  );

  const handlePointerDown = useCallback(
    (e: React.PointerEvent<HTMLDivElement>) => {
      if (!enabled) return;
      // Primary button only; ignore right/middle clicks.
      if (e.button !== 0) return;
      e.preventDefault();
      e.currentTarget.setPointerCapture?.(e.pointerId);
      dragRef.current = { startX: e.clientX, startWidth: width };
      setDragging(true);
    },
    [enabled, width],
  );

  const handlePointerMove = useCallback(
    (e: React.PointerEvent<HTMLDivElement>) => {
      if (!dragRef.current) return;
      const rawDelta = e.clientX - dragRef.current.startX;
      // Right-edge handle: rightward drag widens. Left-edge: leftward widens.
      const delta = side === 'right' ? rawDelta : -rawDelta;
      onResize(clamp(dragRef.current.startWidth + delta));
    },
    [clamp, onResize, side],
  );

  const endDrag = useCallback(
    (e: React.PointerEvent<HTMLDivElement>) => {
      if (!dragRef.current) return;
      e.currentTarget.releasePointerCapture?.(e.pointerId);
      dragRef.current = null;
      setDragging(false);
    },
    [],
  );

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLDivElement>) => {
      if (!enabled) return;
      const growKey = side === 'right' ? 'ArrowRight' : 'ArrowLeft';
      const shrinkKey = side === 'right' ? 'ArrowLeft' : 'ArrowRight';
      let next: number | null = null;
      if (e.key === growKey || e.key === 'ArrowUp') next = width + step;
      else if (e.key === shrinkKey || e.key === 'ArrowDown') next = width - step;
      else if (e.key === 'Home') next = min;
      else if (e.key === 'End') next = max;
      else return;
      e.preventDefault();
      onResize(clamp(next));
    },
    [clamp, enabled, max, min, onResize, side, step, width],
  );

  return {
    dragging,
    handleProps: {
      role: 'separator',
      'aria-orientation': 'vertical',
      'aria-label': label,
      'aria-valuemin': min,
      'aria-valuemax': Number.isFinite(max) ? max : width,
      'aria-valuenow': Math.round(width),
      tabIndex: enabled ? 0 : -1,
      onPointerDown: handlePointerDown,
      onPointerMove: handlePointerMove,
      onPointerUp: endDrag,
      onPointerCancel: endDrag,
      onKeyDown: handleKeyDown,
    },
  };
}

export default useResizable;
