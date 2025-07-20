import { useCallback } from 'react';
import { useToast, ToastAction } from '../features/design-system/components';

interface ErrorOptions {
  title?: string;
  showDetails?: boolean;
  actionLabel?: string;
  onAction?: () => void;
}

export function useErrorHandler() {
  const { toast } = useToast();

  const handleError = useCallback((error: unknown, options: ErrorOptions = {}) => {
    const {
      title = 'Something went wrong',
      showDetails = true,
      actionLabel,
      onAction
    } = options;

    // Extract error message
    let message = 'An unexpected error occurred';
    let details: string | undefined;

    if (error instanceof Error) {
      message = error.message;
      if (showDetails && error.stack) {
        details = error.stack;
      }
    } else if (typeof error === 'string') {
      message = error;
    } else if (error && typeof error === 'object' && 'message' in error) {
      message = String(error.message);
    }

    // Create user-friendly error messages
    const userFriendlyMessages: Record<string, string> = {
      'Network request failed': 'Unable to connect to the server. Please check your internet connection.',
      'Failed to fetch': 'Could not load data. Please try again later.',
      'Unauthorized': 'You need to sign in to access this feature.',
      'Forbidden': 'You don\'t have permission to perform this action.',
      'Not found': 'The requested resource was not found.',
      'Internal server error': 'The server encountered an error. Please try again later.',
      'Timeout': 'The request took too long. Please try again.',
      'Invalid input': 'Please check your input and try again.',
      'Quota exceeded': 'You\'ve reached the usage limit. Please try again later.',
    };

    // Check for known error patterns
    let friendlyMessage = message;
    for (const [pattern, friendly] of Object.entries(userFriendlyMessages)) {
      if (message.toLowerCase().includes(pattern.toLowerCase())) {
        friendlyMessage = friendly;
        break;
      }
    }

    // Show toast notification
    toast({
      title,
      description: friendlyMessage,
      variant: 'destructive',
      action: actionLabel && onAction ? (
        <ToastAction altText={actionLabel} onClick={onAction}>
          {actionLabel}
        </ToastAction>
      ) : undefined,
    });

    // Log error to console in development
    if (process.env.NODE_ENV === 'development') {
      console.error('Error handled:', {
        error,
        message,
        details,
        friendlyMessage
      });
    }
  }, [toast]);

  const handleAsyncError = useCallback((promise: Promise<any>, options?: ErrorOptions) => {
    return promise.catch(error => {
      handleError(error, options);
      throw error; // Re-throw to allow caller to handle if needed
    });
  }, [handleError]);

  return {
    handleError,
    handleAsyncError
  };
}

// Utility function for wrapping async functions with error handling
export function withErrorHandler<T extends (...args: any[]) => Promise<any>>(
  fn: T,
  options?: ErrorOptions
): T {
  return (async (...args: Parameters<T>) => {
    try {
      return await fn(...args);
    } catch (error) {
      const { toast } = useToast();

      // Similar error handling logic as above
      let message = 'An unexpected error occurred';
      if (error instanceof Error) {
        message = error.message;
      } else if (typeof error === 'string') {
        message = error;
      }

      toast({
        title: options?.title || 'Error',
        description: message,
        variant: 'destructive',
      });

      throw error;
    }
  }) as T;
}