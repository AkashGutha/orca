import * as React from 'react'
import { cva, type VariantProps } from 'class-variance-authority'

import { cn } from '../../lib/cn'

const buttonVariants = cva(
  'inline-flex items-center justify-center rounded-md text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-orca-400 disabled:pointer-events-none disabled:opacity-50',
  {
    variants: {
      variant: {
        default: 'bg-orca-600 text-white hover:bg-sky-600',
        secondary: 'bg-sky-100 text-sky-950 hover:bg-sky-200',
        outline: 'border border-sky-200 bg-white/60 text-sky-950 hover:bg-sky-50',
        ghost: 'text-sky-950 hover:bg-sky-50',
        danger: 'bg-red-500 text-white hover:bg-red-400',
      },
      size: {
        default: 'h-10 px-4 py-2',
        sm: 'h-8 px-3',
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

export function Button({ className, variant, size, ...props }: ButtonProps) {
  return <button className={cn(buttonVariants({ variant, size, className }))} {...props} />
}
