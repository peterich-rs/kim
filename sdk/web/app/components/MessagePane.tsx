import { ArrowLeft, ImagePlus, Loader2, MessageSquare, Send, UserPlus, Users } from "lucide-react";
import { useEffect, useMemo, useRef, useState, type ChangeEvent, type FormEvent, type KeyboardEvent } from "react";
import { toast } from "sonner";

import { COPY, memberCount } from "../copy.ts";
import { cn } from "../lib/cn.ts";
import { formatClock } from "../lib/format.ts";
import { ACCEPT_IMAGE, isImageMessage } from "../lib/image.ts";
import { useChat, type ChatMsg } from "../state/ChatProvider.tsx";
import { ImageBubble } from "./ImageBubble.tsx";
import { ImageViewer } from "./ImageViewer.tsx";
import { Avatar, Button, IconTip } from "./ui.tsx";

const GROUP_MS = 5 * 60 * 1000;

function sameGroup(prev: ChatMsg | undefined, cur: ChatMsg): boolean {
  if (!prev || prev.sys || cur.sys) {
    return false;
  }
  return prev.sender === cur.sender && cur.at - prev.at < GROUP_MS;
}

export function MessagePane() {
  const {
    account,
    active,
    activeId,
    messages,
    members,
    membersOpen,
    status,
    send,
    sendImage,
    closeThread,
    toggleMembers,
    socialReady,
    isFriend,
    isOutgoing,
    isIncoming,
    requestFriend,
    acceptFriend,
  } = useChat();
  const [draft, setDraft] = useState("");
  const [sending, setSending] = useState(false);
  const [uploading, setUploading] = useState(false);
  const [viewer, setViewer] = useState<string | null>(null);
  const logRef = useRef<HTMLOListElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const fileRef = useRef<HTMLInputElement>(null);

  const rows = activeId ? (messages[activeId] ?? []) : [];

  useEffect(() => {
    const el = logRef.current;
    if (el) {
      el.scrollTop = el.scrollHeight;
    }
  }, [rows.length, activeId]);

  useEffect(() => {
    inputRef.current?.focus();
    setDraft("");
  }, [activeId]);

  const grouped = useMemo(() => {
    return rows.map((row, i) => ({
      row,
      cont: sameGroup(rows[i - 1], row),
    }));
  }, [rows]);

  async function onSend() {
    const text = draft.trim();
    if (!text || !active || sending) {
      return;
    }
    if (status !== "online") {
      toast.error(COPY.notConnected);
      return;
    }
    setSending(true);
    try {
      await send(text);
      setDraft("");
      const area = inputRef.current;
      if (area) {
        area.style.height = "auto";
      }
    } catch (err) {
      const msg = err instanceof Error ? err.message : "";
      const known = [COPY.notConnected, COPY.notFriends, COPY.blocked, COPY.userNotFound];
      toast.error(known.includes(msg) ? msg : COPY.sendFailed);
    } finally {
      setSending(false);
    }
  }

  function onSubmit(ev: FormEvent) {
    ev.preventDefault();
    void onSend();
  }

  function onKey(ev: KeyboardEvent<HTMLTextAreaElement>) {
    if (ev.key === "Enter" && !ev.shiftKey) {
      ev.preventDefault();
      void onSend();
    }
  }

  function resize(el: HTMLTextAreaElement) {
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, 144)}px`;
  }

  async function onPickImage(ev: ChangeEvent<HTMLInputElement>) {
    const file = ev.target.files?.[0];
    ev.target.value = "";
    if (!file || uploading || sending) {
      return;
    }
    if (status !== "online") {
      toast.error(COPY.notConnected);
      return;
    }
    setUploading(true);
    try {
      await sendImage(file);
    } catch (err) {
      const msg = err instanceof Error ? err.message : "";
      const known = [
        COPY.notConnected,
        COPY.notFriends,
        COPY.blocked,
        COPY.userNotFound,
        COPY.imageTooLarge,
        COPY.imageUnsupported,
      ];
      toast.error(known.includes(msg) ? msg : COPY.imageFailed);
    } finally {
      setUploading(false);
    }
  }

  if (!active) {
    return (
      <div className="hidden min-h-0 flex-1 flex-col items-center justify-center bg-stage text-center md:flex">
        <div className="grid size-16 place-items-center rounded-2xl bg-panel text-brand">
          <MessageSquare className="size-7" />
        </div>
        <h2 className="mt-4 text-lg font-semibold">{COPY.pickConversation}</h2>
        <p className="mt-1 max-w-xs text-sm text-muted">{COPY.pickConversationHint}</p>
      </div>
    );
  }

  const subtitle = active.kind === "group" ? COPY.groupChat : COPY.privateChat;
  const gated =
    active.kind === "user" && socialReady && !isFriend(active.id);

  return (
    <div className="relative flex min-h-0 min-w-0 flex-1">
      <section className="flex min-w-0 flex-1 flex-col bg-stage">
        <header className="flex items-center gap-3 border-b border-line px-3 py-3">
          <Button variant="icon" className="md:hidden" aria-label={COPY.back} onClick={closeThread}>
            <ArrowLeft className="size-4" />
          </Button>
          <Avatar name={active.title} size="sm" />
          <div className="min-w-0 flex-1">
            <h2 className="truncate text-sm font-semibold">{active.title}</h2>
            <p className="text-xs text-muted">
              {active.kind === "group" && members.length > 0 ? memberCount(members.length) : subtitle}
            </p>
          </div>
          {active.kind === "group" ? (
            <IconTip label={COPY.members}>
              <Button variant="icon" aria-label={COPY.members} onClick={toggleMembers}>
                <Users className="size-4" />
              </Button>
            </IconTip>
          ) : null}
        </header>

        <ol ref={logRef} className="msg-scroll flex min-h-0 flex-1 flex-col gap-1 overflow-y-auto px-4 py-4">
          {grouped.length === 0 ? (
            <li className="m-auto text-sm text-muted">{COPY.noMessages}</li>
          ) : (
            grouped.map(({ row, cont }) => {
              if (row.sys) {
                return (
                  <li key={row.key} className="self-center py-1 text-center text-xs text-muted">
                    {row.body}
                  </li>
                );
              }
              const mine = row.sender === account;
              return (
                <li
                  key={row.key}
                  className={cn(
                    "flex max-w-[min(72%,36rem)] gap-2",
                    mine ? "self-end flex-row-reverse" : "self-start",
                    cont && "mt-0",
                  )}
                >
                  {cont ? <span className="size-8 shrink-0" /> : <Avatar name={row.sender} size="sm" />}
                  <div className={cn("min-w-0", mine && "items-end")}>
                    {cont ? null : (
                      <div className={cn("mb-1 flex items-baseline gap-2", mine && "flex-row-reverse")}>
                        <span className="text-xs font-medium text-muted">
                          {mine ? COPY.you : row.sender}
                        </span>
                        <time className="text-[11px] text-muted/70">{formatClock(row.at)}</time>
                      </div>
                    )}
                    {isImageMessage(row.type, row.body, row.extra) ? (
                      <ImageBubble
                        src={row.body}
                        size={row.width > 0 && row.height > 0 ? { w: row.width, h: row.height } : undefined}
                        mine={mine}
                        onOpen={setViewer}
                      />
                    ) : (
                      <div
                        className={cn(
                          "whitespace-pre-wrap break-words rounded-2xl px-3 py-2 text-sm leading-relaxed",
                          mine ? "rounded-tr-md bg-me" : "rounded-tl-md bg-them",
                        )}
                      >
                        {row.body}
                      </div>
                    )}
                  </div>
                </li>
              );
            })
          )}
        </ol>

        {gated ? (
          <div className="border-t border-line bg-panel px-4 py-5 pb-[max(1.25rem,env(safe-area-inset-bottom))]">
            <div className="mx-auto flex max-w-md flex-col items-center rounded-2xl border border-line bg-stage px-5 py-5 text-center">
              <Avatar name={active.title} />
              <p className="mt-3 text-sm font-medium">{COPY.notFriends}</p>
              <p className="mt-1 text-xs leading-relaxed text-muted">
                {isOutgoing(active.id)
                  ? COPY.waitingAccept
                  : isIncoming(active.id)
                    ? COPY.friendRequestToast
                    : COPY.addFriendToChat}
              </p>
              {isOutgoing(active.id) ? (
                <span className="mt-4 text-xs text-muted">{COPY.requested}</span>
              ) : isIncoming(active.id) ? (
                <Button
                  className="mt-4 h-9 px-4 text-xs"
                  onClick={() => {
                    void acceptFriend(active.id).catch((err) =>
                      toast.error(err instanceof Error ? err.message : COPY.sendFailed),
                    );
                  }}
                >
                  {COPY.accept}
                </Button>
              ) : (
                <Button
                  className="mt-4 h-9 px-4 text-xs"
                  onClick={() => {
                    void requestFriend(active.id).catch((err) =>
                      toast.error(err instanceof Error ? err.message : COPY.sendFailed),
                    );
                  }}
                >
                  <UserPlus className="size-3.5" />
                  {COPY.addFriend}
                </Button>
              )}
            </div>
          </div>
        ) : (
          <form
            className="flex items-end gap-2 border-t border-line bg-panel px-3 py-3 pb-[max(0.75rem,env(safe-area-inset-bottom))]"
            onSubmit={onSubmit}
          >
            <input
              ref={fileRef}
              type="file"
              accept={ACCEPT_IMAGE}
              className="hidden"
              onChange={(ev) => {
                void onPickImage(ev);
              }}
            />
            <IconTip label={COPY.pickImage}>
              <Button
                type="button"
                variant="icon"
                className="h-11 w-11"
                disabled={uploading || sending || status !== "online"}
                aria-label={COPY.pickImage}
                onClick={() => fileRef.current?.click()}
              >
                {uploading ? <Loader2 className="size-4 animate-spin" /> : <ImagePlus className="size-4" />}
              </Button>
            </IconTip>
            <textarea
              ref={inputRef}
              rows={1}
              value={draft}
              maxLength={4000}
              onChange={(e) => {
                setDraft(e.target.value);
                resize(e.target);
              }}
              onKeyDown={onKey}
              placeholder={COPY.messagePlaceholder}
              className="max-h-36 min-h-11 flex-1 resize-none rounded-xl border border-line bg-elev px-3 py-2.5 text-sm placeholder:text-muted/70 focus:border-brand focus:outline-none focus:ring-2 focus:ring-brand/30"
            />
            <Button
              type="submit"
              className="h-11 px-4"
              disabled={!draft.trim() || sending || status !== "online"}
              aria-label={COPY.send}
            >
              {sending ? <Loader2 className="size-4 animate-spin" /> : <Send className="size-4" />}
              <span className="hidden sm:inline">{COPY.send}</span>
            </Button>
          </form>
        )}
      </section>

      {active.kind === "group" && membersOpen ? (
        <aside className="absolute inset-y-0 right-0 z-20 flex w-[min(240px,80vw)] flex-col border-l border-line bg-panel md:relative md:z-0">
          <header className="flex items-center justify-between border-b border-line px-4 py-3">
            <h3 className="text-sm font-semibold">{COPY.members}</h3>
            <Button variant="icon" className="md:hidden" aria-label={COPY.close} onClick={toggleMembers}>
              <ArrowLeft className="size-4" />
            </Button>
          </header>
          <ul className="msg-scroll flex-1 overflow-y-auto p-2">
            {members.map((name) => (
              <li key={name} className="flex items-center gap-2 rounded-lg px-2 py-2">
                <Avatar name={name} size="sm" />
                <span className="truncate text-sm">
                  {name}
                  {name === account ? <span className="ml-1 text-xs text-muted">{COPY.you}</span> : null}
                </span>
              </li>
            ))}
          </ul>
        </aside>
      ) : null}

      <ImageViewer src={viewer} open={Boolean(viewer)} onClose={() => setViewer(null)} />
    </div>
  );
}
