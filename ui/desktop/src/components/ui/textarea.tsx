import * as React from 'react'

import { cn } from '../../lib/cn'

export type TextareaProps = React.TextareaHTMLAttributes<HTMLTextAreaElement>

export function Textarea({ className, ...props }: TextareaProps) {
  return (
    <textarea
      className={cn(
        'min-h-24 w-full rounded-md border border-sky-200 bg-white px-2.5 py-2 text-xs leading-5 text-slate-950 shadow-sm placeholder:text-slate-400 focus:border-orca-500 focus:outline-none focus:ring-1 focus:ring-orca-500',
        className,
      )}
      {...props}
    />
  )
}
