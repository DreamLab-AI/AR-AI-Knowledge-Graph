import * as React from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import { cva, type VariantProps } from 'class-variance-authority'
import { cn } from '../../../utils/classNameUtils'
import { animations } from '../animations'
import { Eye, EyeOff, X } from 'lucide-react'

const inputVariants = cva(
  'flex w-full rounded-lg bg-background text-foreground transition-all duration-200 file:border-0 file:bg-transparent file:text-sm file:font-medium placeholder:text-muted-foreground focus-visible:outline-none disabled:cursor-not-allowed disabled:opacity-50',
  {
    variants: {
      variant: {
        default: 'border border-input focus:border-primary focus:ring-2 focus:ring-primary/20',
        filled: 'bg-secondary border border-transparent focus:border-primary focus:bg-background',
        flushed: 'border-0 border-b-2 border-input rounded-none px-0 focus:border-primary',
        ghost: 'border-0 focus:bg-accent/5',
        outlined: 'border-2 border-input focus:border-primary',
      },
      size: {
        xs: 'h-7 px-2 text-xs',
        sm: 'h-8 px-3 text-sm',
        default: 'h-10 px-4 text-base',
        lg: 'h-12 px-5 text-lg',
        xl: 'h-14 px-6 text-xl',
      },
      state: {
        default: '',
        error: 'border-destructive focus:border-destructive focus:ring-destructive/20',
        success: 'border-green-500 focus:border-green-500 focus:ring-green-500/20',
        warning: 'border-yellow-500 focus:border-yellow-500 focus:ring-yellow-500/20',
      },
    },
    defaultVariants: {
      variant: 'default',
      size: 'default',
      state: 'default',
    },
  }
)

export interface InputProps
  extends Omit<React.InputHTMLAttributes<HTMLInputElement>, 'size'>,
    VariantProps<typeof inputVariants> {
  label?: string
  error?: string
  success?: string
  warning?: string
  helper?: string
  icon?: React.ReactNode
  iconPosition?: 'left' | 'right'
  clearable?: boolean
  onClear?: () => void
  animated?: boolean
  floatingLabel?: boolean
}

const Input = React.forwardRef<HTMLInputElement, InputProps>(
  (
    {
      className,
      type = 'text',
      variant,
      size,
      state: stateProp,
      label,
      error,
      success,
      warning,
      helper,
      icon,
      iconPosition = 'left',
      clearable = false,
      onClear,
      animated = true,
      floatingLabel = false,
      value,
      onChange,
      onFocus,
      onBlur,
      disabled,
      ...props
    },
    ref
  ) => {
    const [isFocused, setIsFocused] = React.useState(false)
    const [hasValue, setHasValue] = React.useState(!!value)
    const [showPassword, setShowPassword] = React.useState(false)

    const state = error ? 'error' : success ? 'success' : warning ? 'warning' : stateProp || 'default'
    const message = error || success || warning || helper

    React.useEffect(() => {
      setHasValue(!!value)
    }, [value])

    const handleFocus = (e: React.FocusEvent<HTMLInputElement>) => {
      setIsFocused(true)
      onFocus?.(e)
    }

    const handleBlur = (e: React.FocusEvent<HTMLInputElement>) => {
      setIsFocused(false)
      onBlur?.(e)
    }

    const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
      setHasValue(!!e.target.value)
      onChange?.(e)
    }

    const handleClear = () => {
      const input = (ref as React.RefObject<HTMLInputElement>)?.current;
      if (input) {
        const nativeInputValueSetter = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, "value")!.set;
        nativeInputValueSetter!.call(input, '');
        const event = new Event('input', { bubbles: true });
        input.dispatchEvent(event);
      }
      onClear?.()
    }

    const isSensitive = type === 'password';

    const inputElement = (
      <div className="relative w-full">
        {floatingLabel && label && (
          <motion.label
            className={cn(
              'absolute left-4 text-muted-foreground transition-all duration-200 pointer-events-none',
              'origin-left',
              size === 'xs' && 'text-xs',
              size === 'sm' && 'text-sm',
              size === 'lg' && 'text-lg',
              size === 'xl' && 'text-xl'
            )}
            initial={false}
            animate={{
              y: hasValue || isFocused ? -20 : 0,
              scale: hasValue || isFocused ? 0.85 : 1,
              color: isFocused ? 'var(--color-primary)' : 'var(--color-muted-foreground)',
            }}
            transition={animations.transitions.spring.smooth}
          >
            {label}
          </motion.label>
        )}

        <div className="relative flex items-center">
          {icon && iconPosition === 'left' && (
            <div className="absolute left-3 text-muted-foreground">{icon}</div>
          )}

          <input
            ref={ref}
            type={isSensitive && !showPassword ? 'password' : 'text'}
            className={cn(
              inputVariants({ variant, size, state }),
              icon && iconPosition === 'left' && 'pl-10',
              (icon || isSensitive || clearable) && iconPosition === 'right' && 'pr-10',
              floatingLabel && 'pt-4',
              className
            )}
            value={value}
            onChange={handleChange}
            onFocus={handleFocus}
            onBlur={handleBlur}
            disabled={disabled}
            aria-invalid={state === 'error'}
            aria-describedby={message ? `${props.id}-message` : undefined}
            {...props}
          />

          <div className="absolute right-3 flex items-center gap-2">
            {clearable && hasValue && !disabled && (
              <motion.button
                type="button"
                onClick={handleClear}
                className="text-muted-foreground hover:text-foreground transition-colors"
                initial={{ opacity: 0, scale: 0.8 }}
                animate={{ opacity: 1, scale: 1 }}
                exit={{ opacity: 0, scale: 0.8 }}
                transition={animations.transitions.spring.snappy}
              >
                <X className="h-4 w-4" />
              </motion.button>
            )}
            {isSensitive && (
              <button type="button" onClick={() => setShowPassword(!showPassword)} className="text-muted-foreground hover:text-foreground">
                {showPassword ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
              </button>
            )}
            {icon && iconPosition === 'right' && (
              <div className="text-muted-foreground">{icon}</div>
            )}
          </div>
        </div>

        {animated && variant === 'flushed' && (
          <motion.div
            className="absolute bottom-0 left-0 h-0.5 bg-primary"
            initial={{ scaleX: 0 }}
            animate={{ scaleX: isFocused ? 1 : 0 }}
            transition={animations.transitions.spring.smooth}
            style={{ originX: 0.5 }}
          />
        )}
      </div>
    )

    return (
      <div className="w-full">
        {label && !floatingLabel && (
          <label
            htmlFor={props.id}
            className={cn(
              'block text-sm font-medium mb-1.5',
              state === 'error' && 'text-destructive',
              state === 'success' && 'text-success',
              state === 'warning' && 'text-warning'
            )}
          >
            {label}
          </label>
        )}

        {inputElement}

        <AnimatePresence mode="wait">
          {message && (
            <motion.p
              id={`${props.id}-message`}
              className={cn(
                'mt-1.5 text-sm',
                state === 'error' && 'text-destructive',
                state === 'success' && 'text-green-600',
                state === 'warning' && 'text-yellow-600',
                state === 'default' && 'text-muted-foreground'
              )}
              initial={{ opacity: 0, y: -10 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -10 }}
              transition={animations.transitions.spring.snappy}
            >
              {message}
            </motion.p>
          )}
        </AnimatePresence>
      </div>
    )
  }
)
Input.displayName = 'Input'

export { Input, inputVariants }