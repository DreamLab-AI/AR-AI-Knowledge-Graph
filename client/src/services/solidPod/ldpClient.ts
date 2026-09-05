/**
 * LDP Client
 *
 * Low-level Linked Data Platform operations against a Solid/JSS server:
 * - URL resolution and path canonicalisation
 * - NIP-98 / dev-mode auth header construction
 * - JSON-LD and Turtle resource fetch
 * - PUT, POST, DELETE, HEAD (resourceExists)
 * - JSON-LD coercion helper
 * - sanitizePreferenceKey path-traversal guard
 */

import { createLogger } from '../../utils/loggerConfig';
import { nostrAuth } from '../nostrAuthService';
import { computeAuthHeaders } from '../api/authInterceptor';

const logger = createLogger('SolidPodService:ldp');

export const SOLID_POD_BASE_URL = import.meta.env.VITE_SOLID_POD_URL || '/solid';

// ---------------------------------------------------------------------------
// Types (re-exported so the main service can re-export them)
// ---------------------------------------------------------------------------

export interface JsonLdDocument {
  '@context': string | object;
  '@type'?: string;
  '@id'?: string;
  [key: string]: unknown;
}

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

/**
 * Sanitize a preference key to prevent path traversal.
 * Strips path separators and dot-sequences that could escape the container.
 */
export function sanitizePreferenceKey(key: string): string {
  let sanitized = key.replace(/[\/\\\.]{2,}/g, '');
  sanitized = sanitized.replace(/[\/\\]/g, '-');
  sanitized = sanitized.replace(/^[.\-]+/, '');
  if (!sanitized) {
    throw new Error('Invalid preference key');
  }
  return sanitized;
}

/**
 * Resolve a pod-relative path or absolute URL to a proxy-prefixed URL.
 * Rewrites Docker-internal JSS hostnames to the proxy path.
 */
export function resolvePath(path: string): string {
  if (path.startsWith('http://') || path.startsWith('https://')) {
    const jssPattern =
      /^https?:\/\/[^/]*(?:visionclaw-jss|jss|localhost)[^/]*(?::\d+)?\/(.*)$/;
    const match = path.match(jssPattern);
    if (match) {
      return `${SOLID_POD_BASE_URL}/${match[1]}`;
    }
    return path;
  }

  if (path.startsWith(SOLID_POD_BASE_URL + '/') || path === SOLID_POD_BASE_URL) {
    return path;
  }

  const cleanPath = path.startsWith('/') ? path.slice(1) : path;
  return `${SOLID_POD_BASE_URL}/${cleanPath}`;
}

/**
 * Extract the pathname from a full URL; returns the input unchanged if it is
 * already a path.
 */
export function extractPath(url: string): string {
  try {
    return new URL(url).pathname;
  } catch {
    return url;
  }
}

// ---------------------------------------------------------------------------
// Auth
// ---------------------------------------------------------------------------

/**
 * Fetch wrapper that injects NIP-98 (or dev-mode) auth headers.
 */
export async function fetchWithAuth(
  url: string,
  options: RequestInit = {}
): Promise<Response> {
  const headers = new Headers(options.headers);

  if (nostrAuth.isAuthenticated()) {
    try {
      const method = (options.method || 'GET').toUpperCase();
      const body = typeof options.body === 'string' ? options.body : undefined;
      const absoluteUrl = url.startsWith('http')
        ? url
        : `${window.location.origin}${url}`;
      const authHeaders = await computeAuthHeaders(absoluteUrl, method, body);
      for (const [k, v] of Object.entries(authHeaders)) headers.set(k, v);
    } catch (e) {
      logger.warn('NIP-98 signing failed:', e);
    }
  } else if (nostrAuth.getCurrentUser()) {
    logger.warn(
      'Stale auth session: user exists but cannot sign requests. ' +
        'Please log out and log back in.'
    );
  }

  return fetch(url, { ...options, headers, credentials: 'include' });
}

// ---------------------------------------------------------------------------
// LDP CRUD
// ---------------------------------------------------------------------------

/** Fetch a resource and return it as a parsed JSON-LD document. */
export async function fetchJsonLd(resourcePath: string): Promise<JsonLdDocument> {
  const url = resolvePath(resourcePath);
  const response = await fetchWithAuth(url, {
    headers: { Accept: 'application/ld+json' },
  });

  if (!response.ok) {
    throw new Error(`Failed to fetch ${resourcePath}: ${response.status}`);
  }

  return response.json();
}

/** Fetch a resource as a raw Turtle string. */
export async function fetchTurtle(resourcePath: string): Promise<string> {
  const url = resolvePath(resourcePath);
  const response = await fetchWithAuth(url, {
    headers: { Accept: 'text/turtle' },
  });

  if (!response.ok) {
    throw new Error(`Failed to fetch Turtle ${resourcePath}: ${response.status}`);
  }

  return response.text();
}

/** Create or replace a resource (PUT). */
export async function putResource(
  resourcePath: string,
  data: JsonLdDocument | string,
  contentType: 'application/ld+json' | 'text/turtle' = 'application/ld+json'
): Promise<boolean> {
  const url = resolvePath(resourcePath);
  const body = typeof data === 'string' ? data : JSON.stringify(data);

  const response = await fetchWithAuth(url, {
    method: 'PUT',
    headers: { 'Content-Type': contentType },
    body,
  });

  if (!response.ok) {
    logger.error('PUT failed', { resourcePath, status: response.status });
    return false;
  }

  logger.debug('Resource updated', { resourcePath });
  return true;
}

/** Create a resource in a container (POST). Returns the Location header value. */
export async function postResource(
  containerPath: string,
  data: JsonLdDocument,
  slug?: string
): Promise<string | null> {
  const url = resolvePath(containerPath);
  const headers: Record<string, string> = { 'Content-Type': 'application/ld+json' };
  if (slug) headers['Slug'] = slug;

  const response = await fetchWithAuth(url, {
    method: 'POST',
    headers,
    body: JSON.stringify(data),
  });

  if (!response.ok) {
    logger.error('POST failed', { containerPath, status: response.status });
    return null;
  }

  return response.headers.get('Location');
}

/** Delete a resource. Returns true even if already absent (404). */
export async function deleteResource(resourcePath: string): Promise<boolean> {
  const url = resolvePath(resourcePath);
  const response = await fetchWithAuth(url, { method: 'DELETE' });

  if (!response.ok && response.status !== 404) {
    logger.error('DELETE failed', { resourcePath, status: response.status });
    return false;
  }

  return true;
}

/** Check whether a resource exists (HEAD). */
export async function resourceExists(resourcePath: string): Promise<boolean> {
  const url = resolvePath(resourcePath);
  try {
    const response = await fetchWithAuth(url, { method: 'HEAD' });
    return response.ok;
  } catch {
    return false;
  }
}

// ---------------------------------------------------------------------------
// JSON-LD coercion
// ---------------------------------------------------------------------------

/** Ensure content has a valid JSON-LD @context, wrapping if necessary. */
export function ensureJsonLd(
  content: JsonLdDocument | Record<string, unknown> | string
): JsonLdDocument {
  if (typeof content === 'string') {
    try {
      return ensureJsonLd(JSON.parse(content));
    } catch {
      return {
        '@context': 'https://www.w3.org/ns/ldp',
        '@type': 'Resource',
        content,
      };
    }
  }

  if ('@context' in content && content['@context']) {
    return content as JsonLdDocument;
  }

  const { '@context': _, '@type': __, ...rest } = content as Record<string, unknown>;
  return {
    '@context': {
      '@vocab': 'https://narrativegoldmine.com/ontology#',
      ldp: 'https://www.w3.org/ns/ldp#',
      xsd: 'http://www.w3.org/2001/XMLSchema#',
    },
    '@type': 'Resource',
    ...rest,
  };
}
