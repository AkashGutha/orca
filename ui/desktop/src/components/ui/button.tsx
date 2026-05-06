import * as React from 'react'
import { cva, type VariantProps } from 'class-variance-authority'

import { cn } from '../../lib/cn'

const buttonVariants = cva(
  'inline-flex items-center justify-center rounded-md border text-xs font-semibold leading-none shadow-sm transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-orca-400 disabled:pointer-events-none disabled:opacity-50',
  {
    variants: {
      variant: {
        default: 'border-orca-600 bg-orca-600 text-white hover:border-sky-600 hover:bg-sky-600',
        secondary: 'border-sky-200 bg-sky-50 text-sky-950 hover:bg-sky-100',
        outline: 'border-sky-200 bg-white text-sky-950 hover:bg-sky-50',
        ghost: 'border-transparent bg-transparent text-sky-950 shadow-none hover:bg-sky-50',
        danger: 'border-red-500 bg-red-500 text-white hover:border-red-400 hover:bg-red-400',
      },
      size: {
        default: 'h-8 px-3 py-1.5',
        sm: 'h-7 px-2.5',
      },
    },
    defaultVariants: {
      variant: 'default',
      size: 'default',
    },
  },
)

export type ButtonProps = React.ButtonHTMLAttributes<HTMLButtonElement> &
  VariantProps<typeof buttonVariants>

export const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant, size, ...props }, ref) => (
    <button ref={ref} className={cn(buttonVariants({ variant, size, className }))} {...props} />
  ),
)

Button.displayName = 'Button'
