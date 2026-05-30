import React from "react";
import { cn } from "../../lib/utils";

interface CardProps {
  className?: string;
  padding?: boolean;
  children: React.ReactNode;
}

export default function Card({ className, padding = true, children }: CardProps) {
  return (
    <div
      className={cn(
        "bg-[hsl(var(--bg-card))] border border-[hsl(var(--border))] rounded-lg shadow-sm",
        padding && "p-5",
        className
      )}
    >
      {children}
    </div>
  );
}
