/**
 * attentionHeat — file-attention heat for knowledge/ontology nodes.
 *
 * Every agent action arrives client-side as a 0x23 AGENT_ACTION frame, decoded
 * in `store/websocket/binaryProtocol.ts` and fanned out to two sinks: the
 * embodied beams (`transientBeamStore`) and the `emit('agent-action', …)` event.
 * This module subscribes to that SAME event and keeps a decaying accumulator of
 * how much attention each node is currently receiving, so the 3D layer can heat
 * a node up as agents touch it and let it cool once they move on — the file-
 * attention heatmap idiom rendered in our metadata-texture emissive path rather
 * than a 2D grid.
 *
 * Design:
 *  - `touch(nodeId)` adds +1 raw heat (capped), decaying the prior value to now
 *    first so bursts add coherently.
 *  - Heat decays exponentially with a configurable half-life (default 20s).
 *  - `getHeat(nodeId)` returns the CURRENT decayed heat normalised to 0..1 via a
 *    saturating curve — read-only, it never mutates the accumulator.
 *  - The map is bounded: touches over the cap evict the coldest entry, and a
 *    periodic sweep (singleton only) drops entries that have decayed to nothing.
 *  - A `version` counter + `subscribe` let consumers dirty-mark cheaply.
 *
 * Node-id spaces reconcile via `getActualNodeId`: the wire `targetNodeId` may
 * carry the KNOWLEDGE/ONTOLOGY flag bits (bits 26-31), while a client `node.id`
 * is the masked form. BOTH the write (`touch`) and the read (`getHeat`) mask
 * their key, so a beam targeting `0x40000001` and a gem whose `node.id` is `"1"`
 * resolve to the same heat entry regardless of which form each side holds.
 */

import { getActualNodeId } from '@/types/binaryProtocol';
import { registerEventHandler } from '@/store/websocket/connectionManager';
import type { AgentActionEvent } from '@/services/binaryProtocol/frameTypes';

/** Default half-life for the exponential decay, in milliseconds (20s). */
export const DEFAULT_HEAT_HALF_LIFE_MS = 20_000;
/** Raw heat added per touch. */
export const HEAT_PER_TOUCH = 1;
/** Upper bound on accumulated raw heat so a hammered node still cools promptly. */
export const MAX_RAW_HEAT = 6;
/**
 * Normalisation scale for the saturating 0..1 curve `1 - exp(-raw / SATURATION)`.
 * A single fresh touch reads ~0.49, two ~0.74, three ~0.86 — bright enough to
 * pop against typical recency (0.02–0.37) yet never reaching a flat 1.0.
 */
export const HEAT_SATURATION = 1.5;
/** Raw heat below this is treated as cold: dropped by sweeps, ignored by hasHeat. */
export const COLD_RAW_EPSILON = 0.01;
/** Default cap on tracked nodes; the coldest is evicted when a new touch overflows. */
export const DEFAULT_MAX_HEAT_ENTRIES = 512;

interface HeatEntry {
  /** Raw (un-normalised) heat at `ts`. */
  raw: number;
  /** performance.now()/clock value the raw figure was last decayed to. */
  ts: number;
}

export interface AttentionHeatOptions {
  /** Exponential decay half-life in ms. Default 20 000. */
  halfLifeMs?: number;
  /** Max tracked nodes before coldest-eviction. Default 512. */
  maxEntries?: number;
  /** Injectable clock (ms). Default `performance.now`. Tests pass a controllable fn. */
  now?: () => number;
  /** When false, `touch` is a no-op (feature disabled). Default true. */
  enabled?: boolean;
}

export interface AttentionHeatAccumulator {
  /** Record a single agent touch on a node (raw wire id — masked internally). */
  touch(nodeId: number): void;
  /** Record a batch of touches (one per event) — one version bump for the batch. */
  touchMany(nodeIds: number[]): void;
  /** Current decayed heat for a node, normalised 0..1. Accepts wire number or `node.id` string. */
  getHeat(nodeId: string | number): number;
  /** True while any tracked node still holds meaningful heat. */
  hasHeat(): boolean;
  /** Monotonic counter bumped on every touch batch — cheap dirty-marking. */
  getVersion(): number;
  /** Subscribe to version changes; returns an unsubscribe fn. */
  subscribe(listener: () => void): () => void;
  /** Number of tracked nodes (post-sweep may shrink). */
  size(): number;
  /** Drop cold entries; returns how many were removed. */
  sweep(): number;
  /** Update live tuning. `enabled=false` freezes accumulation without clearing. */
  configure(opts: { halfLifeMs?: number; enabled?: boolean; maxEntries?: number }): void;
  /** Drop all heat. */
  clear(): void;
}

/** Normalise raw heat to a saturating 0..1 value. Exported for tests. */
export function normaliseHeat(raw: number): number {
  if (raw <= 0) return 0;
  return 1 - Math.exp(-raw / HEAT_SATURATION);
}

/** Decay a raw value from `fromTs` to `toTs` under the given half-life. */
function decayRaw(raw: number, fromTs: number, toTs: number, halfLifeMs: number): number {
  const dt = toTs - fromTs;
  if (dt <= 0 || raw <= 0) return raw;
  return raw * Math.pow(0.5, dt / halfLifeMs);
}

/** Resolve any id form (wire number or client `node.id` string) to a stable masked key. */
function keyFor(nodeId: string | number): string {
  const n = typeof nodeId === 'number' ? nodeId : Number(nodeId);
  // Non-numeric ids (rare node types) can never be a 0x23 target — key them
  // verbatim so a stray lookup returns 0 rather than colliding on NaN→0.
  if (!Number.isFinite(n)) return String(nodeId);
  return String(getActualNodeId(n));
}

/**
 * Build a self-contained accumulator. The exported singleton wires this to the
 * live `agent-action` stream; tests construct their own instance with a mock
 * clock so touch/decay/cap/normalise are deterministic.
 */
export function createAttentionHeatAccumulator(
  options: AttentionHeatOptions = {},
): AttentionHeatAccumulator {
  const now = options.now ?? (() => performance.now());
  let halfLifeMs = Math.max(1, options.halfLifeMs ?? DEFAULT_HEAT_HALF_LIFE_MS);
  let maxEntries = Math.max(1, options.maxEntries ?? DEFAULT_MAX_HEAT_ENTRIES);
  let enabled = options.enabled ?? true;

  const entries = new Map<string, HeatEntry>();
  const listeners = new Set<() => void>();
  let version = 0;

  const bump = (): void => {
    version++;
    for (const l of listeners) {
      try { l(); } catch { /* a listener must never break the accumulator */ }
    }
  };

  /** Evict the single coldest (lowest decayed raw) entry — used when over cap. */
  const evictColdest = (t: number): void => {
    let coldestKey: string | null = null;
    let coldestRaw = Infinity;
    for (const [k, e] of entries) {
      const r = decayRaw(e.raw, e.ts, t, halfLifeMs);
      if (r < coldestRaw) { coldestRaw = r; coldestKey = k; }
    }
    if (coldestKey !== null) entries.delete(coldestKey);
  };

  const touch = (nodeId: number): void => {
    if (!enabled) return;
    const t = now();
    const key = keyFor(nodeId);
    const existing = entries.get(key);
    if (existing) {
      const decayed = decayRaw(existing.raw, existing.ts, t, halfLifeMs);
      existing.raw = Math.min(decayed + HEAT_PER_TOUCH, MAX_RAW_HEAT);
      existing.ts = t;
    } else {
      // Enforce the cap BEFORE inserting so size never exceeds maxEntries.
      if (entries.size >= maxEntries) evictColdest(t);
      entries.set(key, { raw: Math.min(HEAT_PER_TOUCH, MAX_RAW_HEAT), ts: t });
    }
  };

  const touchMany = (nodeIds: number[]): void => {
    if (!enabled || nodeIds.length === 0) return;
    for (let i = 0; i < nodeIds.length; i++) touch(nodeIds[i]);
    bump();
  };

  const getHeat = (nodeId: string | number): number => {
    const e = entries.get(keyFor(nodeId));
    if (!e) return 0;
    return normaliseHeat(decayRaw(e.raw, e.ts, now(), halfLifeMs));
  };

  const hasHeat = (): boolean => {
    if (entries.size === 0) return false;
    const t = now();
    for (const e of entries.values()) {
      if (decayRaw(e.raw, e.ts, t, halfLifeMs) > COLD_RAW_EPSILON) return true;
    }
    return false;
  };

  const sweep = (): number => {
    if (entries.size === 0) return 0;
    const t = now();
    let removed = 0;
    for (const [k, e] of entries) {
      if (decayRaw(e.raw, e.ts, t, halfLifeMs) <= COLD_RAW_EPSILON) {
        entries.delete(k);
        removed++;
      }
    }
    return removed;
  };

  return {
    touch(nodeId: number) { touch(nodeId); bump(); },
    touchMany,
    getHeat,
    hasHeat,
    getVersion: () => version,
    subscribe(listener: () => void) {
      listeners.add(listener);
      return () => { listeners.delete(listener); };
    },
    size: () => entries.size,
    sweep,
    configure(opts) {
      if (opts.halfLifeMs !== undefined) halfLifeMs = Math.max(1, opts.halfLifeMs);
      if (opts.maxEntries !== undefined) maxEntries = Math.max(1, opts.maxEntries);
      if (opts.enabled !== undefined) enabled = opts.enabled;
    },
    clear() { entries.clear(); },
  };
}

/**
 * Process-wide singleton read by the render layer (GemNodes). Wired once to the
 * live `agent-action` event below.
 */
export const attentionHeat: AttentionHeatAccumulator = createAttentionHeatAccumulator();

/** Guard so the subscription + sweep are wired exactly once. */
let wired = false;

/**
 * Subscribe the singleton to the decoded 0x23 stream and start a lazy cold-entry
 * sweep. Called at module load; idempotent. Kept as a discrete function (not an
 * inline IIFE) so it is testable and so bundlers cannot drop it as dead code.
 */
export function wireAttentionHeat(): void {
  if (wired) return;
  wired = true;

  registerEventHandler('agent-action', (data: unknown) => {
    const actions = data as AgentActionEvent[] | undefined;
    if (!Array.isArray(actions) || actions.length === 0) return;
    const targets = new Array<number>(actions.length);
    for (let i = 0; i < actions.length; i++) targets[i] = actions[i].targetNodeId;
    attentionHeat.touchMany(targets);
  });

  // Cold entries decay to ~0 on read regardless, but a slow sweep keeps the map
  // tidy during silence and lets hasHeat() flip false promptly so the texture
  // upload settles. setInterval is safe here: the singleton lives for the app.
  // Skip under test so no real timer leaks into the vitest run.
  const isTest = typeof import.meta !== 'undefined' && import.meta.env?.MODE === 'test';
  if (!isTest && typeof setInterval !== 'undefined') {
    setInterval(() => attentionHeat.sweep(), 1000);
  }
}

wireAttentionHeat();
