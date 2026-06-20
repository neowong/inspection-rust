import React, { useCallback } from "react";
import { ChevronUp, ChevronDown } from "lucide-react";
import { cn } from "../../lib/utils";

export interface InputProps extends Omit<React.InputHTMLAttributes<HTMLInputElement>, 'size'> {
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

export interface SelectProps extends Omit<React.SelectHTMLAttributes<HTMLSelectElement>, 'size'> {
  size?: "sm" | "md";
}

/** 与 Input 共用同一套高度尺寸，额外添加 leading-none + py-0 消除浏览器原生
 *  select 内边距，确保放在 Toolbar 里和按钮、搜索框、Input 的视觉高度一致。 */
export function Select({ className, size = "md", children, ...props }: SelectProps) {
  return (
    <select
      className={cn(
        "w-full rounded-md bg-[hsl(var(--bg-card))] border border-[hsl(var(--border))] text-[hsl(var(--text-primary))] outline-none transition-colors duration-150 cursor-pointer",
        "leading-none py-0",
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

// ---- SpinInput (number input with custom +/- buttons) ----

export interface SpinInputProps extends Omit<React.InputHTMLAttributes<HTMLInputElement>, 'size' | 'onChange'> {
  size?: "sm" | "md";
  onChange?: (value: number) => void;
}

const spinSizeClasses = {
  sm: { wrap: "h-7", input: "h-7 text-xs pl-2 pr-0", btn: "h-3.5 w-4", icon: 10 },
  md: { wrap: "h-8", input: "h-8 text-sm pl-2.5 pr-0", btn: "h-4 w-5", icon: 12 },
};

export function SpinInput({ className, size = "md", value, onChange, min, max, step, ...props }: SpinInputProps) {
  const s = spinSizeClasses[size];
  const numVal = typeof value === "string" ? parseFloat(value) : (typeof value === "number" ? value : 0);
  const stepVal = typeof step === "string" ? parseFloat(step) : (typeof step === "number" ? step : 1);
  const minVal = typeof min === "string" ? parseFloat(min) : (typeof min === "number" ? min : undefined);
  const maxVal = typeof max === "string" ? parseFloat(max) : (typeof max === "number" ? max : undefined);

  const bump = useCallback((delta: number) => {
    if (!onChange) return;
    let next = (isNaN(numVal) ? 0 : numVal) + delta;
    if (minVal !== undefined && next < minVal) next = minVal;
    if (maxVal !== undefined && next > maxVal) next = maxVal;
    onChange(next);
  }, [onChange, numVal, minVal, maxVal]);

  const btnBase = cn(
    "flex items-center justify-center text-[hsl(var(--text-tertiary))] hover:text-[hsl(var(--text-primary))] hover:bg-[hsl(var(--bg-hover))] transition-colors",
    "border-l border-[hsl(var(--border))] first:border-b first:border-[hsl(var(--border))] first:rounded-tr-md last:rounded-br-md"
  );

  return (
    <div className={cn("inline-flex rounded-md border border-[hsl(var(--border))] bg-[hsl(var(--bg-card))] overflow-hidden", s.wrap, className)}>
      <input
        type="number"
        value={value}
        onChange={(e) => onChange?.(parseFloat(e.target.value) || 0)}
        min={min} max={max} step={step}
        className={cn(
          "w-full bg-transparent text-[hsl(var(--text-primary))] outline-none",
          "[appearance:textfield] [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:m-0 [&::-webkit-outer-spin-button]:m-0",
          s.input
        )}
        {...props}
      />
      <div className="flex flex-col shrink-0">
        <button
          type="button"
          className={btnBase}
          style={{ height: s.btn, width: s.btn }}
          onClick={() => bump(stepVal)}
          tabIndex={-1}
        >
          <ChevronUp size={s.icon} strokeWidth={2.5} />
        </button>
        <button
          type="button"
          className={btnBase}
          style={{ height: s.btn, width: s.btn }}
          onClick={() => bump(-stepVal)}
          tabIndex={-1}
        >
          <ChevronDown size={s.icon} strokeWidth={2.5} />
        </button>
      </div>
    </div>
  );
}
