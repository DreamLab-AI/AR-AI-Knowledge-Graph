# Unified API Client

This directory contains the new unified API client that replaces the previous competing patterns (`apiService.ts`, `client.ts`, and direct fetch calls).

## Features

- **Centralized HTTP Methods**: All HTTP methods (GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS) with consistent interfaces
- **Authentication Management**: Built-in token management with automatic header injection
- **Request/Response Interceptors**: Configurable middleware for request/response processing
- **Retry Logic**: Automatic retry with exponential backoff for failed requests
- **Timeout Handling**: Configurable request timeouts with abort controller support
- **Type Safety**: Full TypeScript generics support for request/response types
- **Error Handling**: Comprehensive error handling with custom ApiError types
- **Request Cancellation**: Built-in support for canceling pending requests
- **Debug Integration**: Works with the existing debug logging infrastructure

## Usage

### Basic Usage

```typescript
import { unifiedApiClient } from './services/api/UnifiedApiClient';

// GET request
const data = await unifiedApiClient.getData<MyType>('/endpoint');

// POST request
const result = await unifiedApiClient.postData<ResponseType>('/endpoint', payload);

// With full response
const response = await unifiedApiClient.get<MyType>('/endpoint');
console.log(response.status, response.data);
```

### Authentication

```typescript
// Set auth token
unifiedApiClient.setAuthToken('your-jwt-token');

// Remove auth token
unifiedApiClient.removeAuthToken();

// Skip auth for specific request
const response = await unifiedApiClient.get('/public-endpoint', { skipAuth: true });
```

### Interceptors

```typescript
unifiedApiClient.setInterceptors({
  onRequest: (config, url) => {
    console.log('Request:', url);
    return config;
  },
  onResponse: (response) => {
    console.log('Response:', response.status);
    return response;
  },
  onError: (error) => {
    console.error('Request failed:', error.message);
    throw error;
  }
});
```

### Configuration

```typescript
// Configure retry behavior
unifiedApiClient.setRetryConfig({
  maxRetries: 3,
  retryDelay: 1000,
  retryCondition: (error, attempt) => error.status >= 500
});

// Cancel all pending requests
unifiedApiClient.cancelRequests();

// Health check
const isHealthy = await unifiedApiClient.healthCheck();
```

## Migration from Old Patterns

### From `apiService.ts`

```typescript
// Old
const data = await apiService.get<T>('/endpoint');

// New
const data = await unifiedApiClient.getData<T>('/endpoint');
```

### From `client.ts`

```typescript
// Old
const response = await apiClient.get('/endpoint');
const data = response.data;

// New (same interface)
const response = await unifiedApiClient.get('/endpoint');
const data = response.data;
```

### From Direct Fetch

```typescript
// Old
const response = await fetch('/api/endpoint', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify(data)
});
const result = await response.json();

// New
const result = await unifiedApiClient.postData('/endpoint', data);
```

## Files Updated

- `src/api/settingsApi.ts` - Settings API with debouncing and batching
- `src/api/workspaceApi.ts` - Workspace CRUD operations
- `src/api/batchUpdateApi.ts` - Batch node/edge operations
- `src/api/analyticsApi.ts` - GPU-accelerated analytics
- `src/api/optimizationApi.ts` - Graph optimization
- `src/api/exportApi.ts` - Graph export functionality
- `src/services/nostrAuthService.ts` - Nostr authentication
- `src/hooks/useHybridSystemStatus.ts` - System status monitoring
- `src/hooks/useAutoBalanceNotifications.ts` - Auto-balance notifications
- Various bots-related services and utilities

## Benefits

1. **Consistency**: Single API interface across the entire codebase
2. **Maintainability**: Centralized configuration and error handling
3. **Performance**: Built-in retry logic and request optimization
4. **Type Safety**: Full TypeScript support with generics
5. **Debugging**: Integrated logging and error tracking
6. **Flexibility**: Interceptors and configuration options
7. **Production Ready**: Timeout handling, request cancellation, and robust error handling