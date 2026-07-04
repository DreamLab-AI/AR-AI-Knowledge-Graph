---
title: Nostr Authentication Implementation
description: VisionClaw client now enforces Nostr authentication before allowing access to the application. All API requests and WebSocket connections include the user's authentication token and pubkey.
category: how-to
tags:
  - tutorial
  - api
  - api
  - frontend
updated-date: 2025-12-18
difficulty-level: intermediate
---


# Nostr Authentication Implementation

## Overview
VisionClaw client now enforces Nostr authentication before allowing access to the application. All API requests and WebSocket connections include the user's authentication token and pubkey.

## Components Implemented

### 1. **useNostrAuth Hook** (`/client/src/hooks/useNostrAuth.ts`)
- React hook that manages authentication state
- Provides `authenticated`, `isLoading`, `user`, `login()`, `logout()` functions
- Automatically initializes and subscribes to auth state changes
- Handles errors gracefully

### 2. **NostrLoginScreen Component** (`/client/src/components/NostrLoginScreen.tsx`)
- Full-screen authentication UI shown to unauthenticated users
- Checks for NIP-07 extension (Alby, nos2x)
- Shows friendly error messages if no extension found
- Provides extension recommendations with links
- Beautiful gradient design matching app theme

### 3. **LoadingScreen Component** (`/client/src/components/LoadingScreen.tsx`)
- Reusable loading screen with spinner
- Used while checking auth status
- Clean, professional design

## Integration Points

### 4. **App.tsx Updates**
```typescript
// Check auth state first
const { authenticated, isLoading, user } = useNostrAuth();

// Show loading while checking auth
if (isLoading) {
  return <LoadingScreen message="Checking authentication..." />;
}

// Force login if not authenticated
if (!authenticated) {
  return <NostrLoginScreen />;
}

// Sync auth state with settings store
useEffect(() => {
  if (authenticated && user) {
    settingsStore.setAuthenticated(true);
    settingsStore.setUser({
      isPowerUser: user.isPowerUser,
      pubkey: user.pubkey
    });
  }
}, [authenticated, user]);
```

### 5. **Settings API Updates** (`/client/src/api/settingsApi.ts`)
All API endpoints now include Authorization header:
```typescript
const getAuthHeaders = () => {
  const token = nostrAuth.getSessionToken();
  return token ? { Authorization: `Bearer ${token}` } : {};
};

// Applied to all endpoints:
axios.get('/api/settings/all', { headers: getAuthHeaders() })
axios.put('/api/settings/physics', data, { headers: getAuthHeaders() })
// ... and all other endpoints
```

### 6. **WebSocket Service Updates** (`/client/src/services/WebSocketService.ts`)
WebSocket connections now include auth:
```typescript
// Include token in connection URL
const token = nostrAuth.getSessionToken();
const wsUrl = token ? `${this.url}?token=${token}` : this.url;
this.socket = new WebSocket(wsUrl);

// Send auth message after connection
const user = nostrAuth.getCurrentUser();
if (token && user) {
  this.sendMessage('authenticate', {
    token,
    pubkey: user.pubkey
  });
}
```

### 7. **Settings Store Updates** (`/client/src/store/settingsStore.ts`)
- Waits for auth to be ready before initializing
- Stores authenticated state and user info
- All settings operations happen after successful auth

## User Flow

1. **User opens app** → Shows LoadingScreen ("Checking authentication...")
2. **No auth found** → Shows NostrLoginScreen
3. **User clicks "Login with Nostr"** → Extension prompts for signature
4. **Auth successful** → User info synced, app initializes normally
5. **All API calls** → Include `Authorization: Bearer {token}` header
6. **WebSocket connects** → Includes token in URL and sends auth message

## Error Handling

- **No NIP-07 Extension**: Shows error with links to install Alby/nos2x
- **Login Rejected**: Shows error message, allows retry
- **Session Expired**: Auth service detects and shows login screen
- **API 401 Errors**: Will trigger logout and return to login screen

## Security Features

- Token stored in localStorage (nostr_session_token)
- Token included in all API requests via Authorization header
- WebSocket authenticated via URL parameter and explicit auth message
- Pubkey included with all authenticated requests
- Session verification on app startup

## Backend Integration

The backend must:
1. Accept `Authorization: Bearer {token}` headers on all endpoints
2. Verify token validity and extract pubkey
3. Accept WebSocket connections with `?token=` parameter
4. Handle `authenticate` WebSocket message with `{token, pubkey}`
5. Associate all requests/connections with the authenticated pubkey

## File Paths

### New Files Created
- `/client/src/hooks/useNostrAuth.ts` - Auth hook
- `/client/src/components/NostrLoginScreen.tsx` - Login UI
- `/client/src/components/NostrLoginScreen.css` - Login styles
- `/client/src/components/LoadingScreen.tsx` - Loading UI
- `/client/src/components/LoadingScreen.css` - Loading styles

### Modified Files
- `/client/src/app/App.tsx` - Added auth gate
- `/client/src/api/settingsApi.ts` - Added auth headers
- `/client/src/services/WebSocketService.ts` - Added auth to WS
- `/client/src/store/settingsStore.ts` - Already had auth state (no changes needed)

## Testing

To test the implementation:
1. Install Alby or nos2x browser extension
2. Clear localStorage to reset auth state
3. Reload the app
4. Should see login screen
5. Click "Login with Nostr"
6. Approve signature in extension
7. App should load normally
8. Check Network tab - all API requests have Authorization header
9. Check WebSocket connection - includes token parameter

## Next Steps for Backend

The backend needs to implement:
1. Token verification middleware for REST endpoints
2. WebSocket authentication handler
3. User-specific settings storage (keyed by pubkey)
4. Per-user filter preferences
5. Session management and token expiration

---

## Related Documentation

- [Documentation Contributing Guidelines](../../CONTRIBUTING.md)
- [Agent Orchestration Guide](../agent-orchestration.md)
- [Pipeline Operator Runbook](../operations/pipeline-operator-runbook.md)
- [Client-Side Filtering Implementation](filtering-nodes.md)
- [Physics & GPU Engine](../../explanation/physics-gpu-engine.md)

## Notes

- Authentication uses existing `nostrAuthService.ts` singleton
- No changes needed to `settingsStore.ts` - it already had auth state variables
- WebSocket will automatically reconnect with auth if connection drops
- All existing functionality preserved - just gated behind auth now
