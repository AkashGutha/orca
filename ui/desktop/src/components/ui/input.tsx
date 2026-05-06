import * as React from 'react'

import { cn } from '../../lib/cn'

export type InputProps = React.InputHTMLAttributes<HTMLInputElement>

export function Input({ className, ...props }: InputProps) {
  return (
    <input
      className={cn(
        'h-10 w-full rounded-md border border-sky-200 bg-white/80 px-3 py-2 text-sm text-slate-950 placeholder:text-slate-400 focus:border-orca-500 focus:outline-none focus:ring-1 focus:ring-orca-500',
        className,
      )}
      {...props}
    />
  )
}
