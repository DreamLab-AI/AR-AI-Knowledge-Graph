# ADR-011: Universal Authentication Enforcement

**Status**: Accepted
**Date**: 2026-03-07
**Context**: GPT-5.4 audit found 14 HIGH findings for unauthenticated endpoints

## Decision

All WebSocket upgrade paths and mutating REST endpoints MUST enforce authentication
via `RequireAuth` middleware at the route configuration level, not handler level.

### Rules
1. WebSocket upgrades: reject with 401 before upgrade if no valid session/token
2. Mutating REST (POST/PUT/DELETE): `RequireAuth::authenticated()` at scope level
3. No "log and allow" patterns - fail closed
4. Query-string token auth disabled in production
5. `X-Nostr-Pubkey` header validated against active session, not trusted raw

### Exceptions
- Health check endpoints (`/health`, `/api/health`)
- Public shared graph retrieval (`/graph-export/shared/{id}`)
- Static asset routes

## Consequences
- All existing unauthenticated WebSocket handlers must add middleware
- Client must handle 401 on WebSocket upgrade failure
- Dev bypass requires explicit `SETTINGS_AUTH_BYPASS=true` + non-production mode check
