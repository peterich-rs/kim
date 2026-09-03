export function Logo() {
  return (
    <div className="flex items-center gap-2.5">
      <svg viewBox="0 0 32 32" className="size-8 shrink-0 text-primary" aria-hidden="true">
        <rect width="32" height="32" rx="8" fill="currentColor" />
        <path fill="#ffffff" d="M9 8h4.2l4.1 6.6V8H22v16h-4.2l-4.1-6.6V24H9z" />
      </svg>
      <span className="text-base font-semibold tracking-tight">KIM</span>
    </div>
  );
}
