import * as React from 'react'
import * as DialogPrimitive from '@radix-ui/react-dialog'
import { motion, AnimatePresence } from 'framer-motion'
import { cva, type VariantProps } from 'class-variance-authority'
import { cn } from '../../../utils/classNameUtils'
import { animations } from '../animations'
import { X, AlertTriangle, Info, CheckCircle } from 'lucide-react'
import { Button } from './Button'

const modalVariants = cva(
  'fixed z-50 w-full bg-background shadow-2xl',
  {
    variants: {
      size: {
        xs: 'max-w-xs',
        sm: 'max-w-sm',
        md: 'max-w-md',
        lg: 'max-w-lg',
        xl: 'max-w-xl',
        '2xl': 'max-w-2xl',
        '3xl': 'max-w-3xl',
        '4xl': 'max-w-4xl',
        '5xl': 'max-w-5xl',
        full: 'max-w-full h-full',
        auto: 'max-w-fit',
      },
      position: {
        center: 'left-[50%] top-[50%] -translate-x-[50%] -translate-y-[50%] rounded-xl',
        top: 'left-[50%] top-0 -translate-x-[50%] rounded-b-xl',
        bottom: 'left-[50%] bottom-0 -translate-x-[50%] rounded-t-xl',
        left: 'left-0 top-[50%] -translate-y-[50%] h-full rounded-r-xl',
        right: 'right-0 top-[50%] -translate-y-[50%] h-full rounded-l-xl',
      },
    },
    defaultVariants: {
      size: 'md',
      position: 'center',
    },
  }
)

const overlayVariants = {
  initial: { opacity: 0 },
  animate: { opacity: 1 },
  exit: { opacity: 0 },
}

const modalAnimationVariants = {
  center: animations.variants.scale,
  top: animations.variants.slideDown,
  bottom: animations.variants.slideUp,
  left: animations.variants.slideRight,
  right: animations.variants.slideLeft,
}

export interface ModalProps extends VariantProps<typeof modalVariants> {
  open?: boolean
  onOpenChange?: (open: boolean) => void
  children: React.ReactNode
  trigger?: React.ReactNode
  closeOnOverlayClick?: boolean
  closeOnEscape?: boolean
  showCloseButton?: boolean
  overlayBlur?: boolean
  preventScroll?: boolean
}

const Modal = ({
  open,
  onOpenChange,
  children,
  trigger,
  size,
  position = 'center',
  closeOnOverlayClick = true,
  closeOnEscape = true,
  showCloseButton = true,
  overlayBlur = false,
  preventScroll = true,
}: ModalProps) => {
  return (
    <DialogPrimitive.Root open={open} onOpenChange={onOpenChange}>
      {trigger && <DialogPrimitive.Trigger asChild>{trigger}</DialogPrimitive.Trigger>}
      <AnimatePresence>
        {open && (
          <DialogPrimitive.Portal forceMount>
            <DialogPrimitive.Overlay asChild>
              <motion.div
                className={cn(
                  'fixed inset-0 z-50 bg-black/50',
                  overlayBlur && 'backdrop-blur-sm'
                )}
                variants={overlayVariants}
                initial="initial"
                animate="animate"
                exit="exit"
                transition={animations.transitions.easing.easeOut}
                onClick={closeOnOverlayClick ? () => onOpenChange?.(false) : undefined}
              />
            </DialogPrimitive.Overlay>
            <DialogPrimitive.Content
              asChild
              onEscapeKeyDown={closeOnEscape ? undefined : (e) => e.preventDefault()}
            >
              <motion.div
                className={cn(modalVariants({ size, position }))}
                variants={modalAnimationVariants[position]}
                initial="initial"
                animate="animate"
                exit="exit"
                transition={animations.transitions.spring.smooth}
                onClick={(e) => e.stopPropagation()}
              >
                {showCloseButton && (
                  <DialogPrimitive.Close asChild>
                    <Button variant="ghost" size="icon-sm" className="absolute right-4 top-4">
                      <X className="h-4 w-4" />
                      <span className="sr-only">Close</span>
                    </Button>
                  </DialogPrimitive.Close>
                )}
                {children}
              </motion.div>
            </DialogPrimitive.Content>
          </DialogPrimitive.Portal>
        )}
      </AnimatePresence>
    </DialogPrimitive.Root>
  )
}

const ModalHeader = React.forwardRef<
  HTMLDivElement,
  React.HTMLAttributes<HTMLDivElement>
>(({ className, ...props }, ref) => (
  <div
    ref={ref}
    className={cn('flex flex-col space-y-1.5 p-6', className)}
    {...props}
  />
))
ModalHeader.displayName = 'ModalHeader'

const ModalTitle = React.forwardRef<
  React.ElementRef<typeof DialogPrimitive.Title>,
  React.ComponentPropsWithoutRef<typeof DialogPrimitive.Title>
>(({ className, ...props }, ref) => (
  <DialogPrimitive.Title
    ref={ref}
    className={cn('text-lg font-semibold leading-none tracking-tight', className)}
    {...props}
  />
))
ModalTitle.displayName = DialogPrimitive.Title.displayName

const ModalDescription = React.forwardRef<
  React.ElementRef<typeof DialogPrimitive.Description>,
  React.ComponentPropsWithoutRef<typeof DialogPrimitive.Description>
>(({ className, ...props }, ref) => (
  <DialogPrimitive.Description
    ref={ref}
    className={cn('text-sm text-muted-foreground', className)}
    {...props}
  />
))
ModalDescription.displayName = DialogPrimitive.Description.displayName

const ModalBody = React.forwardRef<
  HTMLDivElement,
  React.HTMLAttributes<HTMLDivElement>
>(({ className, ...props }, ref) => (
  <div ref={ref} className={cn('p-6', className)} {...props} />
))
ModalBody.displayName = 'ModalBody'

const ModalFooter = React.forwardRef<
  HTMLDivElement,
  React.HTMLAttributes<HTMLDivElement>
>(({ className, ...props }, ref) => (
  <div
    ref={ref}
    className={cn('flex flex-col-reverse sm:flex-row sm:justify-end sm:space-x-2 p-6', className)}
    {...props}
  />
))
ModalFooter.displayName = 'ModalFooter'

interface ConfirmationModalProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  onConfirm: () => void
  onCancel?: () => void
  title: string
  description?: string
  confirmText?: string
  cancelText?: string
  type?: 'default' | 'danger' | 'warning'
  loading?: boolean
}

const ConfirmationModal = ({
  open,
  onOpenChange,
  onConfirm,
  onCancel,
  title,
  description,
  confirmText = 'Confirm',
  cancelText = 'Cancel',
  type = 'default',
  loading = false,
}: ConfirmationModalProps) => {
  const handleCancel = () => {
    onCancel?.()
    onOpenChange(false)
  }

  const handleConfirm = () => {
    onConfirm()
    if (!loading) {
      onOpenChange(false)
    }
  }

  const iconVariants = {
    default: <Info className="h-6 w-6 text-primary" />,
    danger: <AlertTriangle className="h-6 w-6 text-destructive" />,
    warning: <AlertTriangle className="h-6 w-6 text-yellow-500" />,
  }

  return (
    <Modal open={open} onOpenChange={onOpenChange} size="sm">
      <ModalBody>
        <div className="flex items-start gap-4">
          <motion.div
            initial={{ scale: 0, rotate: -180 }}
            animate={{ scale: 1, rotate: 0 }}
            transition={animations.transitions.spring.bounce}
          >
            {iconVariants[type]}
          </motion.div>
          <div className="flex-1">
            <h3 className="text-lg font-semibold">{title}</h3>
            {description && (
              <p className="mt-2 text-sm text-muted-foreground">{description}</p>
            )}
          </div>
        </div>
      </ModalBody>
      <ModalFooter>
        <Button variant="ghost" onClick={handleCancel} disabled={loading}>
          {cancelText}
        </Button>
        <Button
          variant={type === 'danger' ? 'destructive' : 'default'}
          onClick={handleConfirm}
          loading={loading}
        >
          {confirmText}
        </Button>
      </ModalFooter>
    </Modal>
  )
}

export {
  Modal,
  ModalHeader,
  ModalTitle,
  ModalDescription,
  ModalBody,
  ModalFooter,
  ConfirmationModal,
}