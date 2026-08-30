import * as Dialog from "@radix-ui/react-dialog";
import * as Tooltip from "@radix-ui/react-tooltip";
import { X } from "lucide-react";
import type { ButtonHTMLAttributes, InputHTMLAttributes, ReactNode } from "react";

import { COPY } from "../copy.ts";
import { cn } from "../lib/cn.ts";
import { avatarColor, initial } from "../lib/format.ts";

export function Button({
  variant = "primary",
  className,
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: "primary" | "ghost" | "danger" | "icon";
}) {
  return (
    <button
      className={cn(
        "inline-flex items-center justify-center gap-2 rounded-xl text-sm font-medium transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand disabled:opacity-45",
        variant === "primary" && "bg-brand px-4 py-2.5 text-brand-ink hover:bg-brand/90",
        variant === "ghost" &&
          "border border-line bg-transparent px-3 py-2 text-muted hover:bg-elev hover:text-ink",
        variant === "danger" && "bg-danger/15 px-3 py-2 text-danger hover:bg-danger/25",
        variant === "icon" &&
          "size-9 rounded-lg border border-transparent text-muted hover:bg-elev hover:text-ink",
        className,
      )}
      {...props}
    />
  );
}

export function Field({
  label,
  hint,
  error,
  children,
}: {
  label: string;
  hint?: string;
  error?: string;
  children: ReactNode;
}) {
  return (
    <label className="flex flex-col gap-1.5">
      <span className="text-xs font-medium text-muted">{label}</span>
      {children}
      {error ? (
        <span className="text-xs text-danger">{error}</span>
      ) : hint ? (
        <span className="text-xs text-muted/80">{hint}</span>
      ) : null}
    </label>
  );
}

export function TextInput({ className, ...props }: InputHTMLAttributes<HTMLInputElement>) {
  return (
    <input
      className={cn(
        "h-11 w-full min-w-0 rounded-xl border border-line bg-elev px-3 text-sm text-ink placeholder:text-muted/70 focus:border-brand focus:outline-none focus:ring-2 focus:ring-brand/30",
        className,
      )}
      {...props}
    />
  );
}

export function Avatar({ name, size = "md" }: { name: string; size?: "sm" | "md" | "lg" }) {
  return (
    <span
      className={cn(
        "inline-grid shrink-0 place-items-center rounded-xl font-semibold text-white",
        size === "sm" && "size-8 text-xs",
        size === "md" && "size-10 text-sm",
        size === "lg" && "size-12 text-base",
      )}
      style={{ background: avatarColor(name) }}
      aria-hidden="true"
    >
      {initial(name)}
    </span>
  );
}

export function IconTip({
  label,
  children,
}: {
  label: string;
  children: ReactNode;
}) {
  return (
    <Tooltip.Root>
      <Tooltip.Trigger asChild>{children}</Tooltip.Trigger>
      <Tooltip.Portal>
        <Tooltip.Content
          sideOffset={6}
          className="z-50 rounded-md border border-line bg-elev px-2 py-1 text-xs text-ink shadow-lg"
        >
          {label}
        </Tooltip.Content>
      </Tooltip.Portal>
    </Tooltip.Root>
  );
}

export function Modal({
  open,
  onOpenChange,
  title,
  wide,
  children,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  wide?: boolean;
  children: ReactNode;
}) {
  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-40 bg-black/60 backdrop-blur-[2px]" />
        <Dialog.Content
          aria-describedby={undefined}
          className={cn(
            "fixed left-1/2 top-1/2 z-50 -translate-x-1/2 -translate-y-1/2 rounded-2xl border border-line bg-panel p-5 shadow-2xl focus:outline-none",
            wide ? "w-[min(480px,calc(100vw-24px))]" : "w-[min(420px,calc(100vw-32px))]",
          )}
        >
          <div className="mb-4 flex items-start justify-between gap-3">
            <Dialog.Title className="text-base font-semibold">{title}</Dialog.Title>
            <Dialog.Close asChild>
              <Button variant="icon" aria-label={COPY.close} className="-mr-1 -mt-1">
                <X className="size-4" />
              </Button>
            </Dialog.Close>
          </div>
          {children}
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

export function StatusDot({
  status,
}: {
  status: "connecting" | "online" | "reconnecting" | "offline";
}) {
  const color =
    status === "online"
      ? "bg-online"
      : status === "offline"
        ? "bg-danger"
        : "bg-amber-400";
  return <span className={cn("size-2 rounded-full", color)} aria-hidden="true" />;
}
