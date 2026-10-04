import { CircleNotchIcon } from "@phosphor-icons/react";

import { twMerge } from "@/utils";

const sizeClasses = {
  sm: "h-4 w-4",
  md: "h-6 w-6",
  lg: "h-8 w-8",
} as const;

export interface SpinnerProps {
  size?: "sm" | "md" | "lg";
  className?: string;
}

export function Spinner({ size = "md", className }: SpinnerProps) {
  return (
    <CircleNotchIcon
      weight="bold"
      className={twMerge("animate-spin text-surface-400", sizeClasses[size], className)}
    />
  );
}
