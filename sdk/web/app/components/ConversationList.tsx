import * as DropdownMenu from "@radix-ui/react-dropdown-menu";
import { Hash, LogOut, Plus, Search, User } from "lucide-react";
import { useMemo, useState } from "react";

import { COPY } from "../copy.ts";
import { cn } from "../lib/cn.ts";
import { formatListTime } from "../lib/format.ts";
import { useChat } from "../state/ChatProvider.tsx";
import { Avatar, Button, StatusDot } from "./ui.tsx";

export function ConversationList({
  onNewChat,
  onNewGroup,
  onProfile,
}: {
  onNewChat: () => void;
  onNewGroup: () => void;
  onProfile: () => void;
}) {
  const { account, nickname, status, threads, activeId, openThread, signOut, incomingCount } =
    useChat();
  const [query, setQuery] = useState("");

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) {
      return threads;
    }
    return threads.filter(
      (t) => t.title.toLowerCase().includes(q) || t.id.toLowerCase().includes(q),
    );
  }, [threads, query]);

  const statusLabel =
    status === "online"
      ? COPY.online
      : status === "connecting"
        ? COPY.connecting
        : status === "reconnecting"
          ? COPY.reconnecting
          : COPY.offline;

  return (
    <aside className="flex h-full min-h-0 w-full flex-col bg-panel md:w-[320px] md:border-r md:border-line">
      <header className="flex items-center justify-between gap-2 px-4 pb-2 pt-4">
        <h1 className="text-lg font-semibold">{COPY.conversations}</h1>
        <DropdownMenu.Root>
          <DropdownMenu.Trigger asChild>
            <Button variant="icon" aria-label={COPY.startChat} className="relative">
              <Plus className="size-4" />
              {incomingCount > 0 ? (
                <span className="absolute -right-0.5 -top-0.5 size-2 rounded-full bg-brand" />
              ) : null}
            </Button>
          </DropdownMenu.Trigger>
          <DropdownMenu.Portal>
            <DropdownMenu.Content
              align="end"
              sideOffset={6}
              className="z-50 min-w-40 rounded-xl border border-line bg-elev p-1 shadow-xl"
            >
              <DropdownMenu.Item
                className="flex cursor-pointer items-center gap-2 rounded-lg px-3 py-2 text-sm outline-none data-[highlighted]:bg-panel"
                onSelect={onNewChat}
              >
                <User className="size-4 text-muted" />
                {COPY.contacts}
              </DropdownMenu.Item>
              <DropdownMenu.Item
                className="flex cursor-pointer items-center gap-2 rounded-lg px-3 py-2 text-sm outline-none data-[highlighted]:bg-panel"
                onSelect={onNewGroup}
              >
                <Hash className="size-4 text-muted" />
                {COPY.newGroup}
              </DropdownMenu.Item>
            </DropdownMenu.Content>
          </DropdownMenu.Portal>
        </DropdownMenu.Root>
      </header>

      <div className="px-3 pb-2">
        <label className="relative block">
          <Search className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted" />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={COPY.searchPlaceholder}
            className="h-10 w-full rounded-xl border border-line bg-stage pl-9 pr-3 text-sm placeholder:text-muted/70 focus:border-brand focus:outline-none focus:ring-2 focus:ring-brand/30"
          />
        </label>
      </div>

      <ul className="msg-scroll min-h-0 flex-1 overflow-y-auto px-2 pb-2">
        {filtered.length === 0 ? (
          <li className="px-3 py-16 text-center text-sm text-muted">
            {threads.length === 0 ? (
              <>
                <p className="font-medium text-ink/80">{COPY.noConversations}</p>
                <p className="mt-1">{COPY.noConversationsHint}</p>
              </>
            ) : (
              COPY.noMatch
            )}
          </li>
        ) : (
          filtered.map((t) => (
            <li key={t.id}>
              <button
                type="button"
                onClick={() => openThread(t.id, t.kind, t.title)}
                className={cn(
                  "flex w-full items-center gap-3 rounded-xl px-2 py-2 text-left transition-colors hover:bg-elev",
                  t.id === activeId && "bg-elev",
                )}
              >
                <span className="relative">
                  <Avatar name={t.title} />
                  {t.kind === "group" ? (
                    <span className="absolute -bottom-0.5 -right-0.5 grid size-4 place-items-center rounded-md bg-panel text-muted">
                      <Hash className="size-2.5" />
                    </span>
                  ) : null}
                </span>
                <span className="min-w-0 flex-1">
                  <span className="flex items-baseline justify-between gap-2">
                    <span className="truncate text-sm font-medium">{t.title}</span>
                    {t.lastAt ? (
                      <span className="shrink-0 text-[11px] text-muted">{formatListTime(t.lastAt)}</span>
                    ) : null}
                  </span>
                  <span className="mt-0.5 flex items-center gap-2">
                    <span className="truncate text-xs text-muted">{t.lastBody || COPY.noMessages}</span>
                    {t.unread > 0 ? (
                      <span className="ml-auto grid min-w-5 place-items-center rounded-full bg-brand px-1.5 text-[11px] font-semibold text-brand-ink">
                        {t.unread > 99 ? "99+" : t.unread}
                      </span>
                    ) : null}
                  </span>
                </span>
              </button>
            </li>
          ))
        )}
      </ul>

      <footer className="mt-auto flex items-center gap-3 border-t border-line px-3 py-3">
        <button type="button" onClick={onProfile} className="rounded-xl" aria-label={COPY.profile}>
          <Avatar name={nickname || account || "?"} size="sm" />
        </button>
        <div className="min-w-0 flex-1">
          <p className="truncate text-sm font-medium">{nickname || account}</p>
          <p className="flex items-center gap-1.5 text-xs text-muted">
            <StatusDot status={status} />
            {statusLabel}
          </p>
        </div>
        <Button variant="ghost" className="px-2.5 py-1.5 text-xs" onClick={() => void signOut()}>
          <LogOut className="size-3.5" />
          {COPY.logout}
        </Button>
      </footer>
    </aside>
  );
}
