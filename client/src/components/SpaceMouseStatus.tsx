import React, { useState, useEffect } from 'react';
import { AlertTriangle, Info } from 'lucide-react';
import { SpaceDriver } from '../services/SpaceDriverService';

export const SpaceMouseStatus: React.FC = () => {
  const [isConnected, setIsConnected] = useState(false);
  const isSupported = 'hid' in navigator;
  const isSecureContext = window.isSecureContext;
  const isLocalhost = window.location.hostname === 'localhost' || window.location.hostname === '127.0.0.1';
  const currentUrl = window.location.href;

  useEffect(() => {
    const handleConnect = () => setIsConnected(true);
    const handleDisconnect = () => setIsConnected(false);

    SpaceDriver.addEventListener('connect', handleConnect);
    SpaceDriver.addEventListener('disconnect', handleDisconnect);

    return () => {
      SpaceDriver.removeEventListener('connect', handleConnect);
      SpaceDriver.removeEventListener('disconnect', handleDisconnect);
    };
  }, []);

  // Don't show anything if WebHID is supported and we're in a secure context
  if (isSupported && isSecureContext) {
    return null;
  }

  return (
    <div className="fixed top-4 right-4 z-50 max-w-md">
      {!isSecureContext && (
        <div className="bg-yellow-900/90 backdrop-blur-sm text-yellow-100 p-4 rounded-lg shadow-lg mb-2">
          <div className="flex items-start gap-3">
            <AlertTriangle className="w-5 h-5 flex-shrink-0 mt-0.5" />
            <div>
              <h3 className="font-semibold mb-1">SpaceMouse Requires Secure Context</h3>
              <p className="text-sm mb-2">
                WebHID API requires HTTPS or localhost. You're accessing via: {currentUrl}
              </p>
              <div className="text-xs space-y-1">
                <p className="font-semibold">To enable SpaceMouse, choose one option:</p>
                <ol className="list-decimal list-inside space-y-1 ml-2">
                  <li>Access via <a href="http://localhost:3000" className="underline">http://localhost:3000</a></li>
                  <li>Use HTTPS instead of HTTP</li>
                  <li>In Chrome: chrome://flags → "Insecure origins treated as secure" → Add {window.location.origin}</li>
                </ol>
              </div>
            </div>
          </div>
        </div>
      )}

      {!isSupported && isSecureContext && (
        <div className="bg-blue-900/90 backdrop-blur-sm text-blue-100 p-4 rounded-lg shadow-lg">
          <div className="flex items-start gap-3">
            <Info className="w-5 h-5 flex-shrink-0 mt-0.5" />
            <div>
              <h3 className="font-semibold mb-1">Browser Doesn't Support WebHID</h3>
              <p className="text-sm">
                SpaceMouse requires WebHID API. Please use Chrome or Edge browser.
              </p>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};