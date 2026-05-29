import React from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "../../lib/utils";
import { Loader2 } from "lucide-react";

const buttonVariants = cva(
  "inline-flex items-center justify-center gap-1.5 rounded-md text-sm font-medium transition-all duration-150 whitespace-nowrap focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[hsl(var(--accent))] focus-visible:ring-offset-1 focus-visible:ring-offset-[hsl(var(--bg-content))] disabled:opacity-50 disabled:cursor-not-allowed select-none",
  {
    variants: {
      variant: {
        primary: "bg-[hsl(var(--accent))] text-white hover:bg-[hsl(var(--accent)/0.9)] shadow-sm shadow-[hsl(var(--accent)/0.25)]",
        secondary: "bg-transparent text-[hsl(var(--text-primary))] border border-[hsl(var(--border))] hover:bg-[hsl(var(--bg-hover))]",
        ghost: "bg-transparent text-[hsl(var(--text-secondary))] hover:text-[hsl(var(--text-primary))] hover:bg-[hsl(var(--bg-hover))]",
        danger: "bg-[hsl(var(--danger))] text-white hover:bg-[hsl(var(--danger)/0.9)] shadow-sm shadow-[hsl(var(--danger)/0.25)]",
      },
      size: {
        sm: "h-7 px-2.5 text-xs rounded",
        md: "h-8 px-3.5 text-sm rounded-md",
        icon: "h-8 w-8 p-0",
      },
    },
    defaultVariants: {
      variant: "primary",
      size: "md",
    },
  }
);

export interface ButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {
  loading?: boolean;
}

export default function Button({
  className,
  variant,
  size,
  loading,
  disabled,
  children,
  ...props
}: ButtonProps) {
  return (
    <button
      type="button"
      className={cn(buttonVariants({ variant, size, className }))}
      disabled={disabled || loading}
      {...props}
    >
      {loading && <Loader2 className="h-3.5 w-3.5 animate-spin" />}
      {children}
    </button>
  );
}
