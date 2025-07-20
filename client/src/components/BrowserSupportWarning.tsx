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
    <div className={`bg-yellow-50 border border-yellow-200 rounded-md p-4 ${className}`}>
      <div className="flex">
        <div className="flex-shrink-0">
          <svg className="h-5 w-5 text-yellow-400" viewBox="0 0 20 20" fill="currentColor">
            <path fillRule="evenodd" d="M8.485 2.495c.673-1.167 2.357-1.167 3.03 0l6.28 10.875c.673 1.167-.17 2.625-1.516 2.625H3.72c-1.347 0-2.189-1.458-1.515-2.625L8.485 2.495zM10 5a.75.75 0 01.75.75v3.5a.75.75 0 01-1.5 0v-3.5A.75.75 0 0110 5zm0 9a1 1 0 100-2 1 1 0 000 2z" clipRule="evenodd" />
          </svg>
        </div>
        <div className="ml-3">
          <h3 className="text-sm font-medium text-yellow-800">
            Voice features may not work properly
          </h3>
          <div className="mt-2 text-sm text-yellow-700">
            <ul className="list-disc list-inside space-y-1">
              {warnings.map((warning, index) => (
                <li key={index}>{warning}</li>
              ))}
            </ul>
            {!support.isHttps && (
              <div className="mt-3 text-xs">
                <strong>Solution:</strong> Access this application via HTTPS or localhost for voice features to work.
              </div>
            )}
            {!support.getUserMedia && (
              <div className="mt-3 text-xs">
                <strong>Solution:</strong> Please use a modern browser like Chrome, Firefox, Safari, or Edge.
              </div>
            )}
          </div>
        </div>
        <div className="ml-auto pl-3">
          <div className="-mx-1.5 -my-1.5">
            <button
              type="button"
              className="inline-flex bg-yellow-50 rounded-md p-1.5 text-yellow-500 hover:bg-yellow-100 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-offset-yellow-50 focus:ring-yellow-600"
              onClick={() => setShowWarning(false)}
            >
              <span className="sr-only">Dismiss</span>
              <svg className="h-3 w-3" viewBox="0 0 20 20" fill="currentColor">
                <path d="M6.28 5.22a.75.75 0 00-1.06 1.06L8.94 10l-3.72 3.72a.75.75 0 101.06 1.06L10 11.06l3.72 3.72a.75.75 0 101.06-1.06L11.06 10l3.72-3.72a.75.75 0 00-1.06-1.06L10 8.94 6.28 5.22z" />
              </svg>
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};