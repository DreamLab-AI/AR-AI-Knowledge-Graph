// Error handling components and utilities
export { default as ErrorBoundary, useErrorBoundary } from '../ErrorBoundary';
export { ErrorNotification, ConnectionStatusNotification, SettingsSyncErrorNotification, ValidationErrorNotification } from '../ErrorNotification';

// Hooks
export { useErrorHandler, type ErrorOptions, type ErrorContext } from '../../hooks/useErrorHandler';
export { useWebSocketErrorHandler, reportClientError } from '../../hooks/useWebSocketErrorHandler';

// Managers
export { SettingsRetryManager } from '../../store/settingsRetryManager';

// WebSocket error types
export type { WebSocketErrorFrame } from '../../store/websocketStore';