import * as React from 'react'

import { cn } from '../../lib/cn'

export type InputProps = React.InputHTMLAttributes<HTMLInputElement>

export function Input({ className, ...props }: InputProps) {
  return (
    <input
      className={cn(
        'h-10 w-full rounded-md border border-slate-800 bg-slate-950 px-3 py-2 text-sm text-slate-100 placeholder:text-slate-500 focus:border-orca-400 focus:outline-none focus:ring-1 focus:ring-orca-400',
        className,
      )}
      {...props}
    />
  )
}
