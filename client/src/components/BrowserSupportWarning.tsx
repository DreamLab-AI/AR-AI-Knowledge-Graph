import React, { useState, useEffect } from 'react';
import { AudioInputService } from '../services/AudioInputService';

export interface BrowserSupportWarningProps {
  className?: string;
}

export const BrowserSupportWarning: React.FC<BrowserSupportWarningProps> = ({ className = '' }) => {
  const [support, setSupport] = useState<ReturnType<typeof AudioInputService.getBrowserSupport> | null>(null);
  const [showWarning, setShowWarning] = useState(false);

  useEffect(() => {
    const supportInfo = AudioInputService.getBrowserSupport();
    setSupport(supportInfo);

    // Show warning if any critical features are missing
    const hasIssues = !supportInfo.getUserMedia || !supportInfo.isHttps || !supportInfo.audioContext || !supportInfo.mediaRecorder;
    setShowWarning(hasIssues);
  }, []);

  if (!showWarning || !support) {
    return null;
  }

  const getWarningMessage = () => {
    const issues: string[] = [];

    if (!support.isHttps) {
      issues.push('Secure connection (HTTPS) required');
    }

    if (!support.getUserMedia) {
      issues.push('Microphone access not supported by browser');
    }

    if (!support.audioContext) {
      issues.push('Web Audio API not supported');
    }

    if (!support.mediaRecorder) {
      issues.push('MediaRecorder API not supported');
    }

    return issues;
  };

  const warnings = getWarningMessage();

  return (
    <div className={`bg-yellow-50/90 border border-yellow-200 rounded-md p-2 shadow-sm ${className}`}>
      <div className="flex items-center">
        <div className="flex-shrink-0">
          <svg className="h-4 w-4 text-yellow-400" viewBox="0 0 20 20" fill="currentColor">
            <path fillRule="evenodd" d="M8.485 2.495c.673-1.167 2.357-1.167 3.03 0l6.28 10.875c.673 1.167-.17 2.625-1.516 2.625H3.72c-1.347 0-2.189-1.458-1.515-2.625L8.485 2.495zM10 5a.75.75 0 01.75.75v3.5a.75.75 0 01-1.5 0v-3.5A.75.75 0 0110 5zm0 9a1 1 0 100-2 1 1 0 000 2z" clipRule="evenodd" />
          </svg>
        </div>
        <div className="ml-2 flex-1">
          <p className="text-xs font-medium text-yellow-800">
            Voice features unavailable: {warnings.join(' • ')}
          </p>
        </div>
        <button
          type="button"
          className="ml-2 inline-flex bg-yellow-50 rounded p-1 text-yellow-500 hover:bg-yellow-100 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-offset-yellow-50 focus:ring-yellow-600"
          onClick={() => setShowWarning(false)}
        >
          <span className="sr-only">Dismiss</span>
          <svg className="h-3 w-3" viewBox="0 0 20 20" fill="currentColor">
            <path d="M6.28 5.22a.75.75 0 00-1.06 1.06L8.94 10l-3.72 3.72a.75.75 0 101.06 1.06L10 11.06l3.72 3.72a.75.75 0 101.06-1.06L11.06 10l3.72-3.72a.75.75 0 00-1.06-1.06L10 8.94 6.28 5.22z" />
          </svg>
        </button>
      </div>
    </div>
  );
};