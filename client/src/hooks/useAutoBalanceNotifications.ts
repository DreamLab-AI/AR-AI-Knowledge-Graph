import { useEffect, useRef } from 'react';
import { useToast } from '../features/design-system/components/Toast';
import { useSettingsStore } from '../store/settingsStore';
import { unifiedApiClient } from '../services/api/UnifiedApiClient';
import { createLogger } from '../utils/loggerConfig';

const logger = createLogger('useAutoBalanceNotifications');

interface AutoBalanceNotification {
  message: string;
  timestamp: number;
  severity: 'info' | 'warning' | 'success';
}

export function useAutoBalanceNotifications() {
  const { toast } = useToast();
  const lastTimestampRef = useRef<number>(Date.now());
  const pollingIntervalRef = useRef<NodeJS.Timeout | null>(null);
  
  useEffect(() => {
    
    const checkAndPoll = async () => {
      const settings = useSettingsStore.getState().settings;
      const autoBalanceEnabled = (settings?.visualisation?.graphs?.knowledge?.physics as Record<string, unknown> | undefined)?.autoBalance;
      
      if (!autoBalanceEnabled) {
        
        if (pollingIntervalRef.current) {
          clearInterval(pollingIntervalRef.current);
          pollingIntervalRef.current = null;
        }
        return;
      }
      
      
      if (!pollingIntervalRef.current) {
        pollingIntervalRef.current = setInterval(async () => {
          try {
            const response = await unifiedApiClient.get(`/graph/auto-balance-notifications?since=${lastTimestampRef.current}`);
            const data = response.data;
            if (data.success && data.notifications && data.notifications.length > 0) {
              
              data.notifications.forEach((notification: AutoBalanceNotification) => {
                
                toast({
                  title: notification.severity.charAt(0).toUpperCase() + notification.severity.slice(1),
                  description: notification.message,
                  variant: notification.severity === 'success' ? 'default' : notification.severity === 'warning' ? 'destructive' : 'default',
                  duration: 5000,
                });

                
                if (notification.timestamp > lastTimestampRef.current) {
                  lastTimestampRef.current = notification.timestamp;
                }
              });
            }
          } catch (error) {
            logger.error('Failed to fetch auto-balance notifications:', error);
          }
        }, 2000); 
      }
    };
    
    
    checkAndPoll();
    
    
    const unsubscribe = useSettingsStore.subscribe((state) => {
      checkAndPoll();
    });
    
    
    return () => {
      if (pollingIntervalRef.current) {
        clearInterval(pollingIntervalRef.current);
      }
      unsubscribe();
    };
  }, [toast]);
}