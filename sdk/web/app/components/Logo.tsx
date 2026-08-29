import { cn } from "../lib/cn.ts";

export function Logo({ className, markClassName }: { className?: string; markClassName?: string }) {
  return (
    <div className={cn("flex items-center gap-2.5", className)}>
      <svg
        viewBox="0 0 32 32"
        className={cn("size-8 shrink-0", markClassName)}
        aria-hidden="true"
      >
        <rect width="32" height="32" rx="8" fill="#3ee0c5" />
        <path fill="#04241c" d="M9 8h4.2l4.1 6.6V8H22v16h-4.2l-4.1-6.6V24H9z" />
      </svg>
      <span className="text-lg font-semibold tracking-tight">KIM</span>
    </div>
  );
}
