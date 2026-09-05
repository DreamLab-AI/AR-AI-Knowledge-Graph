/**
 * contextLoader — fetch and cache the JSS ontology resource URL.
 *
 * Owns the base-URL configuration constants and the authenticated
 * HTTP helper so every other sub-module can import from one place.
 */

import { createLogger } from '../../../../utils/loggerConfig';
import { computeAuthHeaders } from '../../../../services/api/authInterceptor';

export const logger = createLogger('JssOntologyService');

export const SOLID_POD_BASE_URL = import.meta.env.VITE_SOLID_POD_URL || '/solid';
export const JSS_WS_URL = import.meta.env.VITE_JSS_WS_URL || null;
export const ONTOLOGY_RESOURCE_PATH =
  import.meta.env.VITE_JSS_ONTOLOGY_PATH || '/public/ontology';

export function getOntologyUrl(): string {
  return `${SOLID_POD_BASE_URL}${ONTOLOGY_RESOURCE_PATH}`;
}

export async function fetchWithAuth(
  url: string,
  options: RequestInit = {}
): Promise<Response> {
  const headers = new Headers(options.headers);

  try {
    const method = (options.method || 'GET').toUpperCase();
    const body = typeof options.body === 'string' ? options.body : undefined;
    const authHeaders = await computeAuthHeaders(url, method, body);
    for (const [key, value] of Object.entries(authHeaders)) {
      headers.set(key, value);
    }
  } catch (e) {
    logger.warn('NIP-98 signing failed:', e);
  }

  return fetch(url, {
    ...options,
    headers,
    credentials: 'include',
  });
}
