import React, { useEffect } from 'react'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/features/design-system/components/Card'
import AuthUIHandler from './AuthUIHandler'
import { initializeAuth } from '../initializeAuthentication'
import { createLogger, createErrorMetadata } from '@/utils/logger'

const logger = createLogger('NostrAuthSection');

const NostrAuthSection: React.FC = () => {

  return (
    // Explicitly set dark background and text for the card to ensure theme consistency
    <Card className="bg-card text-card-foreground border-border">
      <CardHeader>
        <CardTitle>Nostr Authentication</CardTitle>
        <CardDescription>Authenticate with your Nostr key to unlock advanced features.</CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col space-y-2">
        <AuthUIHandler />
      </CardContent>
    </Card>
  )
}

export default NostrAuthSection
