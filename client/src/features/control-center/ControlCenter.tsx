/**
 * ControlCenter — the root overlay that composes the whole shell.
 * design-spec.md §2, §6, §7.1.
 *
 * Accepts the SAME props as the legacy IntegratedControlPanel (ControlPanelProps)
 * so the MainLayout swap (WP5) is a one-line change. Responsibilities:
 *  - pointer-events routing: the wrapper is transparent to the pointer; each
 *    surface re-enables pointer events (canvas stays the hero at rest).
 *  - mounts the dock (MacroBar + Ask affordance), the slide-out SettingsPanel,
 *    and the top-right StatusCluster.
 *  - wires the keyboard map (useControlCenterHotkeys) and reveal flow
 *    (useRevealSetting).
 *  - ports the SpaceDriver wiring verbatim from the legacy panel: connect /
 *    disconnect toggle the orbit controls, buttons stream into status.
 */

import React, { useEffect, useState } from 'react';
import { MessageSquare } from 'lucide-react';
import { SpaceDriver } from '../../services/SpaceDriverService';
import { useSettingsStore } from '../../store/settingsStore';
import type { ControlPanelProps } from '../visualisation/components/ControlPanel/types';
import { GlassDock } from './primitives/GlassDock';
import { MacroBar } from './macros/MacroBar';
import { SettingsPanel } from './panels/SettingsPanel';
import { StatusCluster } from './status/StatusCluster';
import { useControlCenterUI } from './state/useControlCenterUI';
import { useControlCenterHotkeys } from './hooks/useControlCenterHotkeys';
import { useRevealSetting } from './hooks/useRevealSetting';
import './styles/control-center.css';

/** Focus the ported CommandInput (mounted elsewhere today). Broadcasts an event
 *  WP3 can wire, and best-effort focuses a mounted input if one is present. */
function focusCommandInput(): void {
  window.dispatchEvent(new CustomEvent('controlcenter:focus-command-input'));
  const input = document.querySelector<HTMLElement>(
    '[data-command-input] input, [data-command-input] textarea',
  );
  input?.focus();
}

export const ControlCenter: React.FC<ControlPanelProps> = ({
  onOrbitControlsToggle,
  botsData,
  graphData,
}) => {
  const dockCollapsed = useControlCenterUI((s) => s.dockCollapsed);
  const toggleDock = useControlCenterUI((s) => s.toggleDock);

  useControlCenterHotkeys();
  useRevealSetting();

  // --- SpaceDriver / SpacePilot wiring (ported verbatim from IntegratedControlPanel) ---
  const [webHidAvailable, setWebHidAvailable] = useState(false);
  const [spacePilotConnected, setSpacePilotConnected] = useState(false);
  const [spacePilotButtons, setSpacePilotButtons] = useState<string[]>([]);

  useEffect(() => {
    setWebHidAvailable('hid' in navigator);
  }, []);

  useEffect(() => {
    const handleConnect = () => {
      setSpacePilotConnected(true);
      onOrbitControlsToggle?.(false);
    };
    const handleDisconnect = () => {
      setSpacePilotConnected(false);
      setSpacePilotButtons([]);
      onOrbitControlsToggle?.(true);
    };
    const handleButtons = (event: Event) => {
      const buttons = (event as CustomEvent<{ buttons?: string[] }>).detail?.buttons ?? [];
      setSpacePilotButtons(buttons);
    };

    SpaceDriver.addEventListener('connect', handleConnect);
    SpaceDriver.addEventListener('disconnect', handleDisconnect);
    SpaceDriver.addEventListener('buttons', handleButtons);

    return () => {
      SpaceDriver.removeEventListener('connect', handleConnect);
      SpaceDriver.removeEventListener('disconnect', handleDisconnect);
      SpaceDriver.removeEventListener('buttons', handleButtons);
    };
  }, [onOrbitControlsToggle]);

  const handleConnectSpacePilot = async () => {
    try {
      await SpaceDriver.scan();
    } catch {
      /* device selection cancelled / unavailable — no-op */
    }
  };

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
        <StatusCluster
          graphData={graphData}
          botsData={botsData}
          mcpConnected={botsData?.mcpConnected ?? false}
          websocketStatus="connected"
          metadataStatus={(graphData?.nodes?.length ?? 0) > 0 ? 'loaded' : 'loading'}
          webHidAvailable={webHidAvailable}
          spacePilotConnected={spacePilotConnected}
          spacePilotButtons={spacePilotButtons}
          onConnectSpacePilot={handleConnectSpacePilot}
        />
      </div>

      <div style={{ pointerEvents: 'auto' }}>
        <SettingsPanel />
      </div>

      <div style={{ pointerEvents: 'auto' }}>
        <GlassDock collapsed={dockCollapsed} onToggleCollapsed={toggleDock}>
          <MacroBar />
          <button
            type="button"
            data-testid="control-center-ask"
            aria-label="Ask — focus the command input"
            onClick={focusCommandInput}
            className="cc-glass flex items-center gap-1.5 px-3 py-1.5 rounded-full text-xs text-foreground hover:text-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
          >
            <MessageSquare size={14} aria-hidden="true" />
            Ask
          </button>
        </GlassDock>
      </div>
    </div>
  );
};

ControlCenter.displayName = 'ControlCenter';

export default ControlCenter;
