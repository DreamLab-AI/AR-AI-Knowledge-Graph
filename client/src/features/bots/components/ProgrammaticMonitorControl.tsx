import React, { useState, useEffect } from 'react';
import { programmaticMonitor } from '../utils/programmaticMonitor';
import { createLogger } from '../../../utils/logger';
import { Button } from '../../design-system/components/Button';

const logger = createLogger('ProgrammaticMonitorControl');

/**
 * Control panel for programmatic bots monitor
 * Allows starting/stopping the monitor that sends updates via HTTP
 */
export const ProgrammaticMonitorControl: React.FC = () => {
  const [isRunning, setIsRunning] = useState(false);
  const [updateCount, setUpdateCount] = useState(0);

  useEffect(() => {
    // Clean up on unmount
    return () => {
      programmaticMonitor.stop();
    };
  }, []);

  const handleToggle = () => {
    if (isRunning) {
      programmaticMonitor.stop();
      setIsRunning(false);
      logger.info('Programmatic monitor stopped');
    } else {
      programmaticMonitor.start(2000); // Update every 2 seconds
      setIsRunning(true);
      logger.info('Programmatic monitor started');

      // Track updates
      const interval = setInterval(() => {
        setUpdateCount(prev => prev + 1);
      }, 2000);

      // Store interval ID for cleanup
      (window as any)._monitorInterval = interval;
    }
  };

  useEffect(() => {
    // Clean up interval when stopping
    if (!isRunning && (window as any)._monitorInterval) {
      clearInterval((window as any)._monitorInterval);
      delete (window as any)._monitorInterval;
    }
  }, [isRunning]);

  const agents = programmaticMonitor.getAgents();

  return (
    <div className="p-4 bg-background border border-gray-200 dark:border-gray-800 rounded-lg space-y-4">
      <div className="flex items-center justify-between">
        <h3 className="text-lg font-semibold">Programmatic Monitor</h3>
        <Button
          onClick={handleToggle}
          variant={isRunning ? 'destructive' : 'default'}
          size="sm"
        >
          {isRunning ? 'Stop Monitor' : 'Start Monitor'}
        </Button>
      </div>

      <div className="grid grid-cols-2 gap-4 text-sm">
        <div>
          <span className="text-muted-foreground">Status:</span>
          <span className={`ml-2 font-medium ${isRunning ? 'text-green-500' : 'text-yellow-500'}`}>
            {isRunning ? 'Running' : 'Stopped'}
          </span>
        </div>
        <div>
          <span className="text-muted-foreground">Updates Sent:</span>
          <span className="ml-2 font-medium">{updateCount}</span>
        </div>
        <div>
          <span className="text-muted-foreground">Active Agents:</span>
          <span className="ml-2 font-medium">{agents.length}</span>
        </div>
        <div>
          <span className="text-muted-foreground">Update Rate:</span>
          <span className="ml-2 font-medium">2s</span>
        </div>
      </div>

      {isRunning && (
        <div className="text-xs text-muted-foreground">
          Sending bots updates via HTTP API endpoint...
        </div>
      )}
    </div>
  );
};