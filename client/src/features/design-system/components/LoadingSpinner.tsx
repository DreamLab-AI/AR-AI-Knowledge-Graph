import React from 'react'
import { cn } from '../../../utils/cn'
import { Loader2 } from 'lucide-react'

interface LoadingSpinnerProps {
  size?: 'sm' | 'md' | 'lg' | 'xl'
  className?: string
  label?: string
}

export function LoadingSpinner({
  size = 'md',
  className,
  label = 'Loading...',
}: LoadingSpinnerProps) {
  const sizeClasses = {
    sm: 'h-4 w-4',
    md: 'h-6 w-6',
    lg: 'h-8 w-8',
    xl: 'h-12 w-12',
  }

  return (
    <div className={cn('flex items-center justify-center', className)}>
      <Loader2
        className={cn('animate-spin text-primary', sizeClasses[size])}
        role="status"
        aria-label={label}
      />
      <span className="sr-only">{label}</span>
    </div>
  )
}

interface LoadingOverlayProps {
  children: React.ReactNode
  isLoading: boolean
  label?: string
  className?: string
}

export function LoadingOverlay({
  children,
  isLoading,
  label = 'Loading...',
  className,
}: LoadingOverlayProps) {
  return (
    <div className={cn('relative', className)}>
      {children}
      {isLoading && (
        <div className="absolute inset-0 z-10 flex items-center justify-center bg-background/80 backdrop-blur-sm rounded-md">
          <div className="flex flex-col items-center gap-2">
            <LoadingSpinner size="lg" />
            {label && (
              <span className="text-sm text-muted-foreground">{label}</span>
            )}
          </div>
        </div>
      )}
    </div>
  )
}