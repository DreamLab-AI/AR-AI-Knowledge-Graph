/**
 * PushToTalkService — Controls audio routing between agent commands and voice chat.
 *
 * Two modes driven by a single PTT key (default: Space):
 *   PTT held:    Mic audio → VisionClaw /ws/speech → Turbo Whisper STT → agent commands
 *   PTT released: Mic audio → LiveKit SFU → spatial voice chat with other users
 *
 * The service doesn't capture audio itself — it coordinates AudioInputService,
 * VoiceWebSocketService, and LiveKitVoiceService by toggling routing.
 */

import { createLogger } from '../utils/loggerConfig';
import { isCanonicalDid } from '../features/voice/pttAgentBinding';

const logger = createLogger('PushToTalkService');

export type PTTMode = 'push' | 'toggle';
export type PTTState = 'idle' | 'commanding' | 'chatting';

export interface PTTConfig {
  /** Key to use for push-to-talk (default: ' ' for Space) */
  key: string;
  /** 'push' = hold to talk to agents, 'toggle' = press to start/stop */
  mode: PTTMode;
  /** Minimum hold duration (ms) before audio is sent (prevents accidental taps) */
  minHoldDuration: number;
  /** Whether voice chat is active when PTT is NOT held */
  voiceChatEnabled: boolean;
}

const DEFAULT_CONFIG: PTTConfig = {
  key: ' ',
  mode: 'push',
  minHoldDuration: 150,
  voiceChatEnabled: true,
};

type PTTEventCallback = (state: PTTState) => void;

export class PushToTalkService {
  private static instance: PushToTalkService;
  private config: PTTConfig;
  private state: PTTState = 'idle';
  private keyDownTime = 0;
  private listeners: Set<PTTEventCallback> = new Set();
  private boundKeyDown: (e: KeyboardEvent) => void;
  private boundKeyUp: (e: KeyboardEvent) => void;
  private userId: string | null = null;
  private wsNotifyCallback: ((pttActive: boolean, selectedAgentDid: string | null) => void) | null = null;
  /**
   * COM-15 / D6: the selected agent's `did:nostr` this PTT session is bound to.
   * When set, a spoken command is addressed to that agent (governed voice loop);
   * when null, PTT is unbound and a command falls back to the settings assistant.
   * This is the selected-agent binding the register found missing — PTT is no
   * longer globally scoped.
   */
  private selectedAgentDid: string | null = null;

  private constructor(config?: Partial<PTTConfig>) {
    this.config = { ...DEFAULT_CONFIG, ...config };
    this.boundKeyDown = this.handleKeyDown.bind(this);
    this.boundKeyUp = this.handleKeyUp.bind(this);
  }

  static getInstance(config?: Partial<PTTConfig>): PushToTalkService {
    if (!PushToTalkService.instance) {
      PushToTalkService.instance = new PushToTalkService(config);
    }
    return PushToTalkService.instance;
  }

  /** Start listening for PTT key events */
  activate(userId: string): void {
    this.userId = userId;
    document.addEventListener('keydown', this.boundKeyDown);
    document.addEventListener('keyup', this.boundKeyUp);
    logger.info(`PTT activated for user ${userId}, key="${this.config.key}", mode=${this.config.mode}`);

    if (this.config.voiceChatEnabled) {
      this.setState('chatting');
    }
  }

  /** Stop listening and reset state */
  deactivate(): void {
    document.removeEventListener('keydown', this.boundKeyDown);
    document.removeEventListener('keyup', this.boundKeyUp);
    this.setState('idle');
    this.userId = null;
    logger.info('PTT deactivated');
  }

  /**
   * Register a callback to notify the server of PTT state changes. The callback
   * receives the PTT-active flag AND the currently bound agent `did:nostr` (or
   * null), so the server can bind the selected agent onto the session at
   * PTT-start (COM-15 / D6 AC1).
   */
  onServerNotify(callback: (pttActive: boolean, selectedAgentDid: string | null) => void): void {
    this.wsNotifyCallback = callback;
  }

  /**
   * COM-15 / D6: bind (or clear) the selected agent for this PTT session. Driven
   * by graph selection (see `usePushToTalkAgentBinding`). Only a canonical
   * `did:nostr` binds; anything else clears the binding (verify before trust). If
   * PTT is already commanding, the server is re-notified so the new target takes
   * effect immediately.
   */
  setSelectedAgentDid(did: string | null): void {
    const next = isCanonicalDid(did) ? did : null;
    if (next === this.selectedAgentDid) return;
    this.selectedAgentDid = next;
    logger.debug(`PTT bound agent → ${next ?? '(none)'}`);
    if (this.state === 'commanding' && this.wsNotifyCallback) {
      this.wsNotifyCallback(true, this.selectedAgentDid);
    }
  }

  /** The `did:nostr` this PTT session is currently bound to, if any. */
  getSelectedAgentDid(): string | null {
    return this.selectedAgentDid;
  }

  /** True iff PTT is bound to a selected agent (a governed target exists). */
  isBoundToAgent(): boolean {
    return this.selectedAgentDid !== null;
  }

  /** Listen for state changes */
  onStateChange(callback: PTTEventCallback): () => void {
    this.listeners.add(callback);
    return () => this.listeners.delete(callback);
  }

  getState(): PTTState {
    return this.state;
  }

  getUserId(): string | null {
    return this.userId;
  }

  updateConfig(config: Partial<PTTConfig>): void {
    this.config = { ...this.config, ...config };
    logger.info('PTT config updated', this.config);
  }

  /**
   * Returns true if the keyboard event originated from a text/selection control
   * the user is actively interacting with — so PTT must NOT hijack the key.
   *
   * Defensive against shadow DOM (composedPath retargeting), ARIA widgets
   * (combobox/textbox/searchbox), native <select>, and Radix/headless triggers.
   */
  private isEditableTarget(e: KeyboardEvent): boolean {
    const EDITABLE_SELECTOR =
      'input, textarea, select, [contenteditable=""], [contenteditable="true"], ' +
      '[role="textbox"], [role="searchbox"], [role="combobox"]';

    // e.target is retargeted across shadow boundaries; check both the focused
    // element and the true composed-path origin to pierce shadow DOM.
    const composedOrigin = e.composedPath?.()[0] as EventTarget | undefined;
    const candidates: Array<EventTarget | null | undefined> = [
      document.activeElement,
      composedOrigin,
      e.target,
    ];

    for (const node of candidates) {
      if (!(node instanceof HTMLElement)) continue;
      if (node.isContentEditable) return true;
      if (node.closest(EDITABLE_SELECTOR)) return true;
    }

    return false;
  }

  private handleKeyDown(e: KeyboardEvent): void {
    if (e.key !== this.config.key) return;
    if (e.repeat) return; // Ignore key repeat

    // Don't capture PTT when typing in input fields / selection controls
    if (this.isEditableTarget(e)) return;

    e.preventDefault();

    if (this.config.mode === 'push') {
      this.keyDownTime = Date.now();
      this.setState('commanding');
    } else {
      // Toggle mode: press to switch between commanding and chatting
      if (this.state === 'commanding') {
        this.setState(this.config.voiceChatEnabled ? 'chatting' : 'idle');
      } else {
        this.setState('commanding');
      }
    }
  }

  private handleKeyUp(e: KeyboardEvent): void {
    if (e.key !== this.config.key) return;
    if (this.config.mode !== 'push') return; // Only relevant for push mode

    e.preventDefault();

    const holdDuration = Date.now() - this.keyDownTime;

    if (holdDuration < this.config.minHoldDuration) {
      // Too short — treat as accidental tap, revert
      logger.debug(`PTT tap too short (${holdDuration}ms < ${this.config.minHoldDuration}ms), ignoring`);
      this.setState(this.config.voiceChatEnabled ? 'chatting' : 'idle');
      return;
    }

    // Release PTT → switch back to voice chat or idle
    this.setState(this.config.voiceChatEnabled ? 'chatting' : 'idle');
  }

  private setState(newState: PTTState): void {
    if (this.state === newState) return;
    const oldState = this.state;
    this.state = newState;

    logger.debug(`PTT state: ${oldState} → ${newState}`);

    // Notify server of PTT state AND the bound agent, so a PTT-start carries the
    // selected agent's did:nostr onto the server session (COM-15 / D6 AC1).
    if (this.wsNotifyCallback) {
      this.wsNotifyCallback(newState === 'commanding', this.selectedAgentDid);
    }

    // Notify listeners
    this.listeners.forEach(cb => {
      try { cb(newState); } catch (err) {
        logger.error('PTT listener error:', err);
      }
    });
  }
}
