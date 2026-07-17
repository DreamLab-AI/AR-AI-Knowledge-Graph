/**
 * ControlCenter — the root overlay that composes the whole shell.
 * design-spec.md §2, §6, §7.1.
 *
 * Accepts the SAME props as the legacy IntegratedControlPanel (ControlPanelProps)
 * so the MainLayout swap (WP5) is a one-line change. Responsibilities:
 *  - pointer-events routing: the wrapper is transparent to the pointer; each
 *    surface re-enables pointer events (canvas stays the hero at rest).
 *  - mounts the dock (MacroBar + Ask affordance) and the slide-out SettingsPanel.
 *  - wires the keyboard map (useControlCenterHotkeys) and reveal flow
 *    (useRevealSetting).
 *  - owns the camera-control handoff: a SpacePilot connect/disconnect toggles
 *    the orbit controls. The status/telemetry + the SpacePilot connect UI now
 *    live in the unified StatusSurface (mounted by MainLayout), which reads
 *    SpaceDriver/websocket state directly — no status props are threaded here.
 */

import React, { useCallback, useEffect, useRef, useState } from 'react';
import { MessageSquare } from 'lucide-react';
import { SpaceDriver } from '../../services/SpaceDriverService';
import { useSettingsStore } from '../../store/settingsStore';
import type { ControlPanelProps } from '../visualisation/components/ControlPanel/types';
import { CommandInput } from '../visualisation/components/CommandInput';
import { GlassDock } from './primitives/GlassDock';
import { MacroBar } from './macros/MacroBar';
import { SettingsPanel } from './panels/SettingsPanel';
import { useControlCenterUI } from './state/useControlCenterUI';
import { useControlCenterHotkeys } from './hooks/useControlCenterHotkeys';
import { useRevealSetting } from './hooks/useRevealSetting';
import './styles/control-center.css';

export const ControlCenter: React.FC<ControlPanelProps> = ({
  onOrbitControlsToggle,
}) => {
  const dockCollapsed = useControlCenterUI((s) => s.dockCollapsed);
  const toggleDock = useControlCenterUI((s) => s.toggleDock);

  // Ask affordance: the dock button opens the natural-language CommandInput
  // (the settings assistant). It renders only while open, so nothing extra is
  // mounted at rest.
  const [askOpen, setAskOpen] = useState(false);
  const askRef = useRef<HTMLDivElement>(null);

  const toggleAsk = useCallback(() => setAskOpen((open) => !open), []);

  // Focus the input each time the Ask box opens. CommandInput mounts
  // synchronously with this render, so focus on the next frame once it is live.
  useEffect(() => {
    if (!askOpen) return;
    const raf = requestAnimationFrame(() => {
      askRef.current?.querySelector<HTMLInputElement>('input, textarea')?.focus();
    });
    return () => cancelAnimationFrame(raf);
  }, [askOpen]);

  // Realise the ported broadcast contract: `controlcenter:focus-command-input`
  // now opens the command input (previously it focused a phantom, unmounted
  // element) so hotkeys / other surfaces can summon it.
  useEffect(() => {
    const open = () => setAskOpen(true);
    window.addEventListener('controlcenter:focus-command-input', open);
    return () => window.removeEventListener('controlcenter:focus-command-input', open);
  }, []);

  // Esc dismisses the Ask box, scoped to when it is open. The panel owns its own
  // Esc via useControlCenterHotkeys, so this never fights it.
  useEffect(() => {
    if (!askOpen) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setAskOpen(false);
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [askOpen]);

  useControlCenterHotkeys();
  useRevealSetting();

  // Camera-control handoff: connecting a SpacePilot hands the camera to the
  // device (orbit controls off); disconnecting hands it back. The SpacePilot
  // display + connect button live in the unified StatusSurface, which reads
  // SpaceDriver directly; this effect owns only the orbit-controls side effect.
  useEffect(() => {
    const handleConnect = () => onOrbitControlsToggle?.(false);
    const handleDisconnect = () => onOrbitControlsToggle?.(true);
    SpaceDriver.addEventListener('connect', handleConnect);
    SpaceDriver.addEventListener('disconnect', handleDisconnect);
    return () => {
      SpaceDriver.removeEventListener('connect', handleConnect);
      SpaceDriver.removeEventListener('disconnect', handleDisconnect);
    };
  }, [onOrbitControlsToggle]);

  // Dev-only test handle required by the browser-automation phase.
  useEffect(() => {
    if (import.meta.env.DEV) {
      (window as unknown as Record<string, unknown>).__settingsStore = useSettingsStore;
      (window as unknown as Record<string, unknown>).__controlCenterUI = useControlCenterUI;
    }
  }, []);

  return (
    <div className="cc-root" data-testid="control-center" style={{ pointerEvents: 'none' }}>
      <div style={{ pointerEvents: 'auto' }}>
        <SettingsPanel />
      </div>

      <div style={{ pointerEvents: 'auto' }}>
        <GlassDock collapsed={dockCollapsed} onToggleCollapsed={toggleDock}>
          <MacroBar />
          <button
            type="button"
            data-testid="control-center-ask"
            aria-label="Ask — open the command input"
            aria-expanded={askOpen}
            aria-controls="control-center-command-input"
            onClick={toggleAsk}
            className="cc-glass flex items-center gap-1.5 px-3 py-1.5 rounded-full text-xs text-foreground hover:text-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
          >
            <MessageSquare size={14} aria-hidden="true" />
            Ask
          </button>
        </GlassDock>
      </div>

      {/* Ask command input — the ported settings-assistant box. It is
          fixed-position and self-manages its own pointer events, and renders
          only while open (isCollapsed toggles its visibility). */}
      <div ref={askRef} id="control-center-command-input" data-command-input>
        <CommandInput isCollapsed={askOpen} />
      </div>
    </div>
  );
};

ControlCenter.displayName = 'ControlCenter';

export default ControlCenter;
