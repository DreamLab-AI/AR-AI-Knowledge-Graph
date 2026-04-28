/**
 * NL-query client — wires per-descriptor LLM intelligence to /api/nl-query/*.
 * See ADR-061 §LLM call envelope. Caches by (intent_hash, ctx_hash).
 */

import { unifiedApiClient } from '@/services/api/UnifiedApiClient';
import type { Setting, SpineContext } from '../types';

interface TranslateRequest {
  intent: string;
  descriptor: {
    id: string;
    label: string;
    path: ReadonlyArray<string>;
    tier: number;
    category: string;
    current_value: unknown;
    bounds?: { min?: number; max?: number; step?: number };
    examples?: string[];
  };
  session_pubkey?: string;
}

export interface TranslateResponse {
  action: 'set' | 'noop' | 'denied';
  path?: ReadonlyArray<string>;
  value?: unknown;
  summary_after?: string;
  explanation?: string;
  confidence?: number;
  reason?: string;
}

export interface ExplainResponse {
  explanation: string;
}

interface ExamplesResponse {
  examples: string[];
}

const cache = new Map<string, TranslateResponse>();

function makeKey(req: TranslateRequest): string {
  return `${req.intent}|${req.descriptor.id}|${JSON.stringify(req.descriptor.current_value)}`;
}

export async function translateIntent<T>(
  intent: string,
  descriptor: Setting<T>,
  currentValue: T,
  ctx: SpineContext
): Promise<TranslateResponse> {
  const req: TranslateRequest = {
    intent,
    descriptor: {
      id: descriptor.id,
      label: descriptor.label,
      path: descriptor.path,
      tier: descriptor.tier,
      category: descriptor.category,
      current_value: currentValue,
      bounds: descriptor.llm?.bounds,
      examples: descriptor.llm?.examples,
    },
    session_pubkey: ctx.pubkey,
  };

  const key = makeKey(req);
  const cached = cache.get(key);
  if (cached) return cached;

  try {
    const response = await unifiedApiClient.post<TranslateResponse>(
      '/api/spine-nl/translate',
      req
    );
    if (response?.data) {
      cache.set(key, response.data);
      return response.data;
    }
    return { action: 'noop', reason: 'empty_response' };
  } catch (err: any) {
    return {
      action: 'denied',
      reason: err?.message ?? 'request_failed',
    };
  }
}

export async function explainDescriptor<T>(
  descriptor: Setting<T>,
  currentValue: T
): Promise<ExplainResponse> {
  try {
    const response = await unifiedApiClient.post<ExplainResponse>(
      '/api/nl-query/explain',
      {
        id: descriptor.id,
        label: descriptor.label,
        path: descriptor.path,
        current_value: currentValue,
        explain_prompt: descriptor.llm?.explainPrompt,
      }
    );
    if (response?.data?.explanation) return response.data;
  } catch {
    // fall through to local copy
  }
  return {
    explanation:
      descriptor.llm?.explainPrompt ??
      `${descriptor.label} (${descriptor.path.join('.')}). Use the editor to adjust.`,
  };
}

export async function fetchExamples<T>(descriptor: Setting<T>): Promise<string[]> {
  if (descriptor.llm?.examples?.length) return [...descriptor.llm.examples];
  try {
    const response = await unifiedApiClient.post<ExamplesResponse>(
      '/api/nl-query/examples',
      { id: descriptor.id, label: descriptor.label }
    );
    return response?.data?.examples ?? [];
  } catch {
    return [];
  }
}

export function clearNlCache(): void {
  cache.clear();
}
