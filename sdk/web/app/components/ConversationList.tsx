import { Ellipsis, Hash, LogOut, Plus, Search, UserPlus, Users, VolumeX } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useDebounceValue } from "usehooks-ts";

import { COPY } from "../copy.ts";
import { formatListTime } from "../lib/format.ts";
import { cn } from "../lib/utils.ts";
import { useChat } from "../state/ChatProvider.tsx";
import { GhostIconButton, UserAvatar } from "./ui.tsx";
import { Badge } from "./ui/badge.tsx";
import { Button } from "./ui/button.tsx";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "./ui/dropdown-menu.tsx";
import { InputGroup, InputGroupAddon, InputGroupInput } from "./ui/input-group.tsx";
import { Item, ItemContent, ItemDescription, ItemMedia, ItemTitle } from "./ui/item.tsx";
import { ScrollArea } from "./ui/scroll-area.tsx";
import { Skeleton } from "./ui/skeleton.tsx";

export function ConversationList({
  onNewChat,
  onAddFriend,
  onNewGroup,
  onProfile,
}: {
  onNewChat: () => void;
  onAddFriend: () => void;
  onNewGroup: () => void;
  onProfile: () => void;
}) {
  const {
    account,
    nickname,
    status,
    threads,
    activeId,
    openThread,
    signOut,
    incomingCount,
    inboxReady,
    muteThread,
  } = useChat();
  const [query, setQuery] = useState("");
  const [debounced] = useDebounceValue(query, 120);
  const [focusIndex, setFocusIndex] = useState(0);
  const [kbNav, setKbNav] = useState(false);

  const filtered = useMemo(() => {
    const q = debounced.trim().toLowerCase();
    if (!q) {
      return threads;
    }
    return threads.filter(
      (t) =>
        t.title.toLowerCase().includes(q) ||
        t.id.toLowerCase().includes(q) ||
        t.lastBody.toLowerCase().includes(q),
    );
  }, [threads, debounced]);

  useEffect(() => {
    setFocusIndex((i) => Math.min(i, Math.max(0, filtered.length - 1)));
  }, [filtered.length]);

  useEffect(() => {
    function onKey(ev: KeyboardEvent) {
      const t = ev.target;
      if (t instanceof HTMLInputElement || t instanceof HTMLTextAreaElement) {
        return;
      }
      if (filtered.length === 0) {
        return;
      }
      if (ev.key === "j" || ev.key === "ArrowDown") {
        ev.preventDefault();
        setKbNav(true);
        setFocusIndex((i) => Math.min(filtered.length - 1, i + 1));
      } else if (ev.key === "k" || ev.key === "ArrowUp") {
        ev.preventDefault();
        setKbNav(true);
        setFocusIndex((i) => Math.max(0, i - 1));
      } else if (ev.key === "Enter") {
        const row = filtered[focusIndex];
        if (row) {
          openThread(row.id, row.kind, row.title);
        }
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [filtered, focusIndex, openThread]);

  const statusLabel =
    status === "online"
      ? COPY.online
      : status === "connecting"
        ? COPY.connecting
        : status === "reconnecting"
          ? COPY.reconnecting
          : COPY.offline;

  const showSkeleton = !inboxReady && threads.length === 0;

  return (
    <aside className="flex h-full min-h-0 w-full flex-col border-r border-sidebar-border bg-sidebar text-sidebar-foreground md:w-80">
      <div className="flex items-center justify-between px-4 pt-4 pb-2">
        <h1 className="text-lg font-semibold tracking-tight">{COPY.conversations}</h1>
        <div className="flex items-center gap-0.5">
          <GhostIconButton label={COPY.contacts} onClick={onNewChat}>
            <span className="relative">
              <Users />
              {incomingCount > 0 ? (
                <Badge className="absolute -top-2 -right-2 h-4 min-w-4 px-1 text-[10px]" variant="default">
                  {incomingCount > 9 ? "9+" : incomingCount}
                </Badge>
              ) : null}
            </span>
          </GhostIconButton>
          <GhostIconButton label={COPY.startChat} onClick={onAddFriend}>
            <UserPlus />
          </GhostIconButton>
          <GhostIconButton label={COPY.newGroup} onClick={onNewGroup}>
            <Plus />
          </GhostIconButton>
        </div>
      </div>

      <div className="px-3 pb-2">
        <InputGroup>
          <InputGroupAddon>
            <Search />
          </InputGroupAddon>
          <InputGroupInput
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={COPY.searchPlaceholder}
            aria-label={COPY.searchPlaceholder}
          />
        </InputGroup>
      </div>

      <ScrollArea className="min-h-0 flex-1">
        <div className="flex flex-col gap-0.5 px-2 pb-2">
          {showSkeleton
            ? Array.from({ length: 8 }, (_, i) => (
                <div key={i} className="flex h-16 items-center gap-3 px-2">
                  <Skeleton className="size-8 rounded-full" />
                  <div className="flex-1 space-y-2">
                    <Skeleton className="h-3 w-1/2" />
                    <Skeleton className="h-3 w-4/5" />
                  </div>
                </div>
              ))
            : filtered.length === 0
              ? (
                <div className="px-3 py-16 text-center">
                  <p className="text-sm font-medium">
                    {threads.length === 0 ? COPY.noConversations : COPY.noMatch}
                  </p>
                  {threads.length === 0 ? (
                    <>
                      <p className="mt-1 text-sm text-muted-foreground">{COPY.noConversationsHint}</p>
                      <button
                        type="button"
                        onClick={onAddFriend}
                        className="mt-3 text-sm font-medium text-primary hover:underline"
                      >
                        {COPY.addFriend}
                      </button>
                    </>
                  ) : null}
                </div>
              )
              : filtered.map((t, i) => (
                  <div key={t.id} className="group/row relative">
                    <Item
                      render={<button type="button" />}
                      size="sm"
                      className={cn(
                        "h-16 rounded-xl border-transparent pr-8 shadow-none",
                        "hover:bg-muted/60",
                        "focus-visible:border-transparent focus-visible:ring-0",
                        t.muted && "opacity-70",
                        t.id === activeId && "bg-muted hover:bg-muted",
                        kbNav && i === focusIndex && t.id !== activeId && "bg-muted/50",
                      )}
                      onClick={() => {
                        setFocusIndex(i);
                        setKbNav(false);
                        openThread(t.id, t.kind, t.title);
                      }}
                    >
                      <ItemMedia>
                        <span className="relative">
                          <UserAvatar name={t.title} />
                          {t.kind === "group" ? (
                            <span className="absolute -right-0.5 -bottom-0.5 grid size-4 place-items-center rounded-md bg-sidebar text-muted-foreground">
                              <Hash className="size-3" />
                            </span>
                          ) : null}
                        </span>
                      </ItemMedia>
                      <ItemContent>
                        <div className="flex items-baseline justify-between gap-2">
                          <ItemTitle className={cn("truncate", t.unread > 0 && "font-bold")}>
                            {t.title}
                          </ItemTitle>
                          {t.lastAt ? (
                            <span className="shrink-0 text-[11px] text-muted-foreground">
                              {formatListTime(t.lastAt)}
                            </span>
                          ) : null}
                        </div>
                        <ItemDescription className="flex items-center gap-1.5">
                          {t.muted ? <VolumeX className="size-3.5 shrink-0" /> : null}
                          <span className="min-w-0 flex-1 truncate">{t.lastBody || COPY.noMessages}</span>
                          {t.unread > 0 ? (
                            <span
                              aria-label={`${t.unread}`}
                              className="ml-auto grid h-5 min-w-5 place-items-center rounded-full bg-unread px-1.5 text-[11px] font-bold text-primary-foreground"
                            >
                              {t.unread > 99 ? "99+" : t.unread}
                            </span>
                          ) : null}
                        </ItemDescription>
                      </ItemContent>
                    </Item>
                    <DropdownMenu>
                      <DropdownMenuTrigger
                        render={
                          <Button
                            type="button"
                            variant="ghost"
                            size="icon-xs"
                            className="absolute top-1/2 right-1.5 -translate-y-1/2 opacity-0 group-hover/row:opacity-100"
                            aria-label={COPY.muteChat}
                          />
                        }
                      >
                        <Ellipsis />
                      </DropdownMenuTrigger>
                      <DropdownMenuContent align="end">
                        <DropdownMenuItem onClick={() => muteThread(t.id, !t.muted)}>
                          {t.muted ? COPY.unmuteChat : COPY.muteChat}
                        </DropdownMenuItem>
                      </DropdownMenuContent>
                    </DropdownMenu>
                  </div>
                ))}
        </div>
      </ScrollArea>

      <div className="flex items-center gap-2.5 border-t border-sidebar-border px-3 py-2.5">
        <GhostIconButton label={COPY.profile} onClick={onProfile}>
          <UserAvatar name={nickname || account || "?"} />
        </GhostIconButton>
        <div className="min-w-0 flex-1">
          <p className="truncate text-sm font-medium">{nickname || account}</p>
          <p className="text-xs text-muted-foreground">{statusLabel}</p>
        </div>
        <GhostIconButton label={COPY.logout} onClick={() => void signOut()}>
          <LogOut />
        </GhostIconButton>
      </div>
    </aside>
  );
}
