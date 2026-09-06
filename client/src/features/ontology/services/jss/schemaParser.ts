/**
 * schemaParser — JSON-LD fetch with caching.
 *
 * Handles both application/ld+json and text/turtle content negotiation,
 * plus the in-memory TTL cache shared across formats.
 */

import { debugState } from '../../../../utils/clientDebugState';
import { createErrorMetadata } from '../../../../utils/loggerConfig';
import { fetchWithAuth, getOntologyJsonLdUrl, getOntologyTurtleUrl, logger } from './contextLoader';

export interface JsonLdContext {
  '@vocab'?: string;
  [key: string]: string | object | undefined;
}

export interface JsonLdOntology {
  '@context': JsonLdContext | string;
  '@graph'?: JsonLdNode[];
  '@id'?: string;
  '@type'?: string | string[];
  [key: string]: unknown;
}

export interface JsonLdNode {
  '@id': string;
  '@type'?: string | string[];
  'rdfs:label'?: string | { '@value': string; '@language'?: string };
  'rdfs:subClassOf'?: { '@id': string } | Array<{ '@id': string }>;
  'rdfs:comment'?: string | { '@value': string };
  'owl:disjointWith'?: { '@id': string } | Array<{ '@id': string }>;
  'rdfs:domain'?: { '@id': string };
  'rdfs:range'?: { '@id': string };
  [key: string]: unknown;
}

export interface FetchOptions {
  skipCache?: boolean;
  timeout?: number;
}

export interface SchemaCache {
  jsonLd: JsonLdOntology | null;
  turtle: string | null;
  timestamp: number;
  readonly ttlMs: number;
}

export function makeSchemaCache(): SchemaCache {
  return { jsonLd: null, turtle: null, timestamp: 0, ttlMs: 60_000 };
}

export function isCacheValid(cache: SchemaCache): boolean {
  if (!cache.jsonLd) return false;
  return Date.now() - cache.timestamp < cache.ttlMs;
}

export function invalidateCache(cache: SchemaCache): void {
  cache.jsonLd = null;
  cache.turtle = null;
  cache.timestamp = 0;
}

export async function fetchJsonLd(
  cache: SchemaCache,
  metrics: { fetchCount: number; cacheHitCount: number; lastFetchDurationMs: number },
  options: FetchOptions = {}
): Promise<JsonLdOntology> {
  const { skipCache = false, timeout = 30_000 } = options;

  if (!skipCache && isCacheValid(cache)) {
    metrics.cacheHitCount++;
    if (debugState.isEnabled()) logger.debug('Returning cached JSON-LD ontology');
    return cache.jsonLd!;
  }

  const startTime = performance.now();
  const url = getOntologyJsonLdUrl();
  const controller = new AbortController();
  const timeoutId = setTimeout(() => controller.abort(), timeout);

  try {
    const response = await fetchWithAuth(url, {
      headers: { Accept: 'application/ld+json' },
      signal: controller.signal,
    });
    clearTimeout(timeoutId);

    if (!response.ok) {
      throw new Error(`Failed to fetch ontology: ${response.status} ${response.statusText}`);
    }

    const data: JsonLdOntology = await response.json();
    cache.jsonLd = data;
    cache.timestamp = Date.now();
    metrics.lastFetchDurationMs = performance.now() - startTime;
    metrics.fetchCount++;

    if (debugState.isEnabled()) {
      logger.info('Fetched JSON-LD ontology', {
        durationMs: metrics.lastFetchDurationMs.toFixed(2),
        graphSize: data['@graph']?.length || 0,
      });
    }

    return data;
  } catch (error) {
    clearTimeout(timeoutId);
    if (error instanceof Error && error.name === 'AbortError') {
      throw new Error(`Ontology fetch timeout after ${timeout}ms`);
    }
    logger.error('Failed to fetch JSON-LD ontology', createErrorMetadata(error));
    throw error;
  }
}

export async function fetchTurtle(
  cache: SchemaCache,
  metrics: { fetchCount: number; cacheHitCount: number; lastFetchDurationMs: number },
  options: FetchOptions = {}
): Promise<string> {
  const { skipCache = false, timeout = 30_000 } = options;

  if (!skipCache && cache.turtle && isCacheValid(cache)) {
    metrics.cacheHitCount++;
    if (debugState.isEnabled()) logger.debug('Returning cached Turtle ontology');
    return cache.turtle;
  }

  const startTime = performance.now();
  const url = getOntologyTurtleUrl();
  const controller = new AbortController();
  const timeoutId = setTimeout(() => controller.abort(), timeout);

  try {
    const response = await fetchWithAuth(url, {
      headers: { Accept: 'text/turtle' },
      signal: controller.signal,
    });
    clearTimeout(timeoutId);

    if (!response.ok) {
      throw new Error(`Failed to fetch Turtle ontology: ${response.status} ${response.statusText}`);
    }

    const data = await response.text();
    cache.turtle = data;
    cache.timestamp = Date.now();
    metrics.lastFetchDurationMs = performance.now() - startTime;
    metrics.fetchCount++;

    if (debugState.isEnabled()) {
      logger.info('Fetched Turtle ontology', {
        durationMs: metrics.lastFetchDurationMs.toFixed(2),
        sizeBytes: data.length,
      });
    }

    return data;
  } catch (error) {
    clearTimeout(timeoutId);
    if (error instanceof Error && error.name === 'AbortError') {
      throw new Error(`Turtle fetch timeout after ${timeout}ms`);
    }
    logger.error('Failed to fetch Turtle ontology', createErrorMetadata(error));
    throw error;
  }
}
