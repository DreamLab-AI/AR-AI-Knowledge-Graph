import * as React from 'react'
import { Search, X } from 'lucide-react'
import { cn } from '../../../utils/cn'
import { Input, InputProps } from './Input'

export interface SearchInputProps extends Omit<InputProps, 'type' | 'onChange'> {
  value: string
  onChange: (value: string) => void
  onSearch?: (value: string) => void
  onClear?: () => void
}

const SearchInput = React.forwardRef<HTMLInputElement, SearchInputProps>(
  ({ className, value, onChange, onSearch, onClear, onKeyDown, ...props }, ref) => {
    const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
      if (e.key === 'Enter' && onSearch) {
        onSearch(value)
      }
      onKeyDown?.(e)
    }

    const handleClear = () => {
      onChange('')
      onClear?.()
    }

    return (
      <div className={cn('relative w-full', className)}>
        <Input
          ref={ref}
          type="search"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          onKeyDown={handleKeyDown}
          icon={<Search className="h-4 w-4" />}
          iconPosition="left"
          className="pl-10 pr-10" // Make space for icons
          {...props}
        />
        {value && (
          <button
            type="button"
            onClick={handleClear}
            className="absolute right-3 top-1/2 -translate-y-1/2 p-1 text-muted-foreground hover:text-foreground transition-colors"
            aria-label="Clear search"
          >
            <X className="h-4 w-4" />
          </button>
        )}
      </div>
    )
  }
)
SearchInput.displayName = 'SearchInput'

export { SearchInput }