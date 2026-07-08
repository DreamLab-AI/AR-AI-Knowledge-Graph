// COM-15 / D6 / M5 (PRD-023 WP-5): the consumer that wires the previously
// call-site-less `PushToTalkService` into the live voice path.
//
// Responsibilities:
//   1. Own the `PushToTalkService` lifecycle (activate on mount, deactivate on
//      unmount) — this is the call site the register found missing (M5).
//   2. Bind graph selection → the selected agent's `did:nostr` (D6): a
//      `visionclaw:node-selected` event resolves to a DID and binds it onto the
//      PTT session; deselecting an agent unbinds it.
//   3. Thread PTT-start to the server: on every PTT edge the bound DID is sent
//      via `set_ptt` so the server session carries the target (D6 AC1).
//   4. Dispatch a final transcript down the GOVERNED path when commanding and
//      bound — a signed 31402 to `/v1/voice-intent` — never the settings
//      assistant for a bound command (falsification clause 2).
//
// The heavy STT capture stays in `useVoiceInteraction`; this hook returns a
// `handleTranscription` the caller feeds from that hook's `onTranscription`, so
// the two compose without a parallel capture path.

import { useCallback, useEffect, useRef } from 'react';
import { PushToTalkService } from '../../services/PushToTalkService';
import { VoiceWebSocketService } from '../../services/VoiceWebSocketService';
import { createLogger } from '../../utils/loggerConfig';
import {
  resolveSelectedAgentDid,
  shouldDispatchGoverned,
  type NodeSelectedDetail,
} from './pttAgentBinding';

const logger = createLogger('usePushToTalkAgentBinding');

export interface PushToTalkAgentBindingReturn {
  /**
   * Feed this from `useVoiceInteraction({ onTranscription })`. On a FINAL
   * transcript, if PTT is commanding and bound to an agent DID, it dispatches the
   * governed voice command; otherwise it is inert.
   */
  handleTranscription: (text: string, isFinal: boolean) => void;
}

export function usePushToTalkAgentBinding(userId = 'operator'): PushToTalkAgentBindingReturn {
  const pttRef = useRef<PushToTalkService | null>(null);

  useEffect(() => {
    const ptt = PushToTalkService.getInstance();
    const voice = VoiceWebSocketService.getInstance();
    pttRef.current = ptt;

    ptt.activate(userId);

    // Graph selection → PTT agent binding (D6). resolveSelectedAgentDid returns
    // null for a non-agent node, which unbinds — so PTT never targets a
    // non-agent.
    const onSelected = (e: Event) => {
      const detail = (e as CustomEvent).detail as NodeSelectedDetail | null;
      ptt.setSelectedAgentDid(resolveSelectedAgentDid(detail));
    };
    window.addEventListener('visionclaw:node-selected', onSelected);

    // Every PTT edge notifies the server of {active, boundDid}, so the server
    // session carries the selected agent at PTT-start.
    ptt.onServerNotify((active, did) => {
      voice.setPtt(active, did);
    });

    return () => {
      window.removeEventListener('visionclaw:node-selected', onSelected);
      ptt.deactivate();
      pttRef.current = null;
    };
  }, [userId]);

  const handleTranscription = useCallback((text: string, isFinal: boolean) => {
    if (!isFinal) return;
    const trimmed = text.trim();
    if (!trimmed) return;

    const ptt = pttRef.current;
    if (!ptt) return;
    const did = ptt.getSelectedAgentDid();

    if (shouldDispatchGoverned(ptt.getState(), did)) {
      logger.info(`governed voice command → ${did}: "${trimmed}"`);
      try {
        VoiceWebSocketService.getInstance().sendVoiceCommand(trimmed, did);
      } catch (err) {
        logger.error('governed voice dispatch failed:', err);
      }
    }
  }, []);

  return { handleTranscription };
}
