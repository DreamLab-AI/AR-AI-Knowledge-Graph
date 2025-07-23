import React from 'react';
import { useSpacePilot } from '../hooks/useSpacePilot';
import { Button } from '../../design-system/components/Button';
import { Loader2 } from 'lucide-react';

interface SpacePilotConnectButtonProps {
  className?: string;
}

export const SpacePilotConnectButton: React.FC<SpacePilotConnectButtonProps> = ({ className }) => {
  const { isSupported, isConnected, connect, disconnect } = useSpacePilot();
  const [isConnecting, setIsConnecting] = React.useState(false);

  // Don't render if WebHID is not supported
  if (!isSupported || !window.isSecureContext) {
    return null;
  }

  const handleClick = async () => {
    if (isConnected) {
      disconnect();
    } else {
      setIsConnecting(true);
      try {
        await connect();
      } finally {
        setIsConnecting(false);
      }
    }
  };

  return (
    <Button
      onClick={handleClick}
      disabled={isConnecting}
      variant={isConnected ? 'outline' : 'default'}
      className={className}
    >
      {isConnecting && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
      {isConnected ? 'Disconnect SpaceMouse' : 'Connect SpaceMouse'}
    </Button>
  );
};