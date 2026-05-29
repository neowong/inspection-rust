import React from "react";
import { cn } from "../../lib/utils";

export interface InputProps extends React.InputHTMLAttributes<HTMLInputElement> {
  size?: "sm" | "md";
}

const sizeClasses = {
  sm: "h-7 text-xs px-2",
  md: "h-8 text-sm px-2.5",
};

export default function Input({ className, size = "md", ...props }: InputProps) {
  return (
    <input
      className={cn(
        "w-full rounded-md bg-[hsl(var(--bg-card))] border border-[hsl(var(--border))] text-[hsl(var(--text-primary))] placeholder:text-[hsl(var(--text-tertiary))] outline-none transition-colors duration-150",
        "focus:border-[hsl(var(--accent))] focus:ring-2 focus:ring-[hsl(var(--accent)/0.2)]",
        "disabled:opacity-50 disabled:cursor-not-allowed",
        sizeClasses[size],
        className
      )}
      {...props}
    />
  );
}

export interface SelectProps extends React.SelectHTMLAttributes<HTMLSelectElement> {
  size?: "sm" | "md";
}

export function Select({ className, size = "md", children, ...props }: SelectProps) {
  return (
    <select
      className={cn(
        "w-full rounded-md bg-[hsl(var(--bg-card))] border border-[hsl(var(--border))] text-[hsl(var(--text-primary))] outline-none transition-colors duration-150 cursor-pointer",
        "focus:border-[hsl(var(--accent))] focus:ring-2 focus:ring-[hsl(var(--accent)/0.2)]",
        "disabled:opacity-50 disabled:cursor-not-allowed",
        sizeClasses[size],
        className
      )}
      {...props}
    >
      {children}
    </select>
  );
}
