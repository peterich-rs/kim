import {
  ArrowLeft,
  ChevronDown,
  CircleAlert,
  ImageIcon,
  MessageCircle,
  Send,
  UserPlus,
  Users,
} from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ChangeEvent,
  type ClipboardEvent,
  type DragEvent,
  type FormEvent,
  type KeyboardEvent,
} from "react";
import { Virtuoso, type VirtuosoHandle } from "react-virtuoso";
import { toast } from "sonner";

import { COPY, memberCount, newCountLabel } from "../copy.ts";
import { formatClock, formatDateSep, startOfDay } from "../lib/format.ts";
import { ACCEPT_IMAGE, isImageMessage } from "../lib/image.ts";
import { cn } from "../lib/utils.ts";
import { useChat, type ChatMsg } from "../state/ChatProvider.tsx";
import { ImageBubble } from "./ImageBubble.tsx";
import { ImageViewer } from "./ImageViewer.tsx";
import { GhostIconButton, UserAvatar } from "./ui.tsx";
import { Badge } from "./ui/badge.tsx";
import { Bubble, BubbleContent } from "./ui/bubble.tsx";
import { Button } from "./ui/button.tsx";
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "./ui/empty.tsx";
import { InputGroup, InputGroupAddon, InputGroupButton, InputGroupTextarea } from "./ui/input-group.tsx";
import {
  Message,
  MessageAvatar,
  MessageContent,
  MessageFooter,
  MessageHeader,
} from "./ui/message.tsx";
import { ScrollArea } from "./ui/scroll-area.tsx";
import { Spinner } from "./ui/spinner.tsx";

const GROUP_MS = 5 * 60 * 1000;
const START_INDEX = 100_000;

function sameGroup(prev: ChatMsg | undefined, cur: ChatMsg | undefined): boolean {
  if (!prev || !cur || prev.sys || cur.sys) {
    return false;
  }
  if (startOfDay(new Date(prev.at)) !== startOfDay(new Date(cur.at))) {
    return false;
  }
  return prev.sender === cur.sender && cur.at - prev.at < GROUP_MS;
}

function DatePill({ label }: { label: string }) {
  return (
    <span className="inline-flex rounded-full bg-muted px-2.5 py-1 text-xs font-medium text-muted-foreground">
      {label}
    </span>
  );
}

function MessageRow({
  item,
  mine,
  isGroup,
  meLabel,
  onRetry,
  onOpenImage,
}: {
  item: { row: ChatMsg; cont: boolean; last: boolean; showDate: boolean };
  mine: boolean;
  isGroup: boolean;
  meLabel: string;
  onRetry: (key: string) => void;
  onOpenImage: (src: string) => void;
}) {
  const { row, cont, last, showDate } = item;
  const align = mine ? "end" : "start";
  const variant = row.status === "failed" ? "destructive" : mine ? "default" : "secondary";
  return (
    <div className={cn("mx-auto max-w-3xl px-4", showDate ? "pt-4" : cont ? "pt-1" : "pt-3", last && "pb-1")}>
      {showDate ? (
        <div className="mb-3 flex justify-center">
          <DatePill label={formatDateSep(row.at)} />
        </div>
      ) : null}
      <Message align={align}>
        {mine ? null : (
          <MessageAvatar>{last ? <UserAvatar name={row.sender} /> : <span className="size-8" />}</MessageAvatar>
        )}
        <MessageContent>
          {!cont && isGroup ? <MessageHeader>{mine ? meLabel : row.sender}</MessageHeader> : null}
          {isImageMessage(row.type, row.body, row.extra) ? (
            <Bubble variant={variant} align={align}>
              <BubbleContent className="p-0">
                <ImageBubble
                  src={row.body}
                  size={row.width > 0 && row.height > 0 ? { w: row.width, h: row.height } : undefined}
                  mine={mine}
                  last={last}
                  onOpen={onOpenImage}
                />
              </BubbleContent>
            </Bubble>
          ) : (
            <Bubble variant={variant} align={align}>
              <BubbleContent className="whitespace-pre-wrap">{row.body}</BubbleContent>
            </Bubble>
          )}
          {last || row.status === "sending" || row.status === "failed" ? (
            <MessageFooter>
              {row.status === "sending"
                ? COPY.sending
                : row.status === "failed"
                  ? null
                  : formatClock(row.at)}
              {row.status === "failed" ? (
                <Button
                  type="button"
                  variant="destructive"
                  size="xs"
                  onClick={() => onRetry(row.key)}
                >
                  <CircleAlert />
                  {COPY.retrySend}
                </Button>
              ) : null}
            </MessageFooter>
          ) : null}
        </MessageContent>
      </Message>
    </div>
  );
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
    retryMessage,
    loadOlder,
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
  const [dragging, setDragging] = useState(false);
  const [atBottom, setAtBottom] = useState(true);
  const [newCount, setNewCount] = useState(0);
  const [firstItemIndex, setFirstItemIndex] = useState(START_INDEX);
  const virtuosoRef = useRef<VirtuosoHandle>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const fileRef = useRef<HTMLInputElement>(null);
  const loadingOlderRef = useRef(false);
  const prevLenRef = useRef(0);
  const atBottomRef = useRef(true);
  atBottomRef.current = atBottom;

  const rows = activeId ? (messages[activeId] ?? []) : [];

  useEffect(() => {
    setDraft("");
    setNewCount(0);
    setAtBottom(true);
    setFirstItemIndex(START_INDEX);
    prevLenRef.current = 0;
    inputRef.current?.focus();
  }, [activeId]);

  useEffect(() => {
    if (!activeId) {
      return;
    }
    const prev = prevLenRef.current;
    if (rows.length > prev && prev > 0) {
      const appended = rows.length - prev;
      const last = rows[rows.length - 1];
      if (last && last.sender !== account && !atBottomRef.current) {
        setNewCount((n) => n + appended);
      }
    }
    prevLenRef.current = rows.length;
  }, [rows.length, activeId, account, rows]);

  const grouped = useMemo(() => {
    return rows.map((row, i) => ({
      row,
      cont: sameGroup(rows[i - 1], row),
      last: !sameGroup(row, rows[i + 1]),
      showDate: i === 0 || startOfDay(new Date(rows[i - 1]!.at)) !== startOfDay(new Date(row.at)),
    }));
  }, [rows]);

  const jumpBottom = useCallback(() => {
    virtuosoRef.current?.scrollToIndex({
      index: Math.max(0, grouped.length - 1),
      align: "end",
      behavior: "smooth",
    });
    setNewCount(0);
    setAtBottom(true);
  }, [grouped.length]);

  async function onStartReached() {
    if (!activeId || loadingOlderRef.current) {
      return;
    }
    loadingOlderRef.current = true;
    try {
      const n = await loadOlder(activeId);
      if (n > 0) {
        setFirstItemIndex((v) => v - n);
      }
    } finally {
      loadingOlderRef.current = false;
    }
  }

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
    } catch (err) {
      const msg = err instanceof Error ? err.message : "";
      const known = [COPY.notConnected, COPY.notFriends, COPY.blocked, COPY.userNotFound];
      if (known.includes(msg)) {
        toast.error(msg);
      }
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

  async function deliverImage(file: File) {
    if (uploading || sending) {
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

  async function onPickImage(ev: ChangeEvent<HTMLInputElement>) {
    const file = ev.target.files?.[0];
    ev.target.value = "";
    if (file) {
      await deliverImage(file);
    }
  }

  function onPaste(ev: ClipboardEvent) {
    const files = ev.clipboardData?.files;
    if (!files?.length) {
      return;
    }
    const image = Array.from(files).find((f) => f.type.startsWith("image/"));
    if (image) {
      ev.preventDefault();
      void deliverImage(image);
    }
  }

  function onDrop(ev: DragEvent) {
    ev.preventDefault();
    setDragging(false);
    const file = ev.dataTransfer.files[0];
    if (file && file.type.startsWith("image/")) {
      void deliverImage(file);
    }
  }

  if (!active) {
    return (
      <div className="hidden min-h-0 flex-1 items-center justify-center bg-canvas px-6 md:flex">
        <Empty>
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <MessageCircle />
            </EmptyMedia>
            <EmptyTitle>{COPY.pickConversation}</EmptyTitle>
            <EmptyDescription>{COPY.pickConversationHint}</EmptyDescription>
          </EmptyHeader>
        </Empty>
      </div>
    );
  }

  const subtitle = active.kind === "group" ? COPY.groupChat : COPY.privateChat;
  const gated = active.kind === "user" && socialReady && !isFriend(active.id);

  return (
    <div
      className="relative flex min-h-0 min-w-0 flex-1"
      onDragOver={(ev) => {
        ev.preventDefault();
        setDragging(true);
      }}
      onDragLeave={() => setDragging(false)}
      onDrop={onDrop}
    >
      <div className="flex min-w-0 flex-1 flex-col bg-canvas">
        <header className="flex min-h-14 items-center gap-3 border-b border-border bg-background px-3 py-2">
          <GhostIconButton label={COPY.back} onClick={closeThread} className="md:hidden">
            <ArrowLeft />
          </GhostIconButton>
          <UserAvatar name={active.title} />
          <div className="min-w-0 flex-1">
            <p className="truncate text-sm font-semibold">{active.title}</p>
            <p className="text-xs text-muted-foreground">
              {active.kind === "group" && members.length > 0 ? memberCount(members.length) : subtitle}
            </p>
          </div>
          {active.kind === "group" ? (
            <GhostIconButton label={COPY.members} onClick={toggleMembers}>
              <Users />
            </GhostIconButton>
          ) : null}
        </header>

        <div className="relative min-h-0 flex-1">
          <Virtuoso
            ref={virtuosoRef}
            style={{ height: "100%" }}
            data={grouped}
            firstItemIndex={firstItemIndex}
            initialTopMostItemIndex={Math.max(0, grouped.length - 1)}
            followOutput={(bottom) => (bottom ? "smooth" : false)}
            atBottomStateChange={(bottom) => {
              setAtBottom(bottom);
              if (bottom) {
                setNewCount(0);
              }
            }}
            startReached={() => {
              void onStartReached();
            }}
            increaseViewportBy={200}
            itemContent={(_index, item) => {
              const { row } = item;
              if (row.sys) {
                return (
                  <div className="mx-auto max-w-3xl px-4 py-3 text-center text-xs text-muted-foreground">
                    {row.body}
                  </div>
                );
              }
              return (
                <MessageRow
                  item={item}
                  mine={row.sender === account}
                  isGroup={active.kind === "group"}
                  meLabel={COPY.you}
                  onRetry={(key) => {
                    void retryMessage(key).catch((err) =>
                      toast.error(err instanceof Error ? err.message : COPY.sendFailed),
                    );
                  }}
                  onOpenImage={setViewer}
                />
              );
            }}
            components={{
              Header: () => <div className="h-2" />,
              Footer: () => <div className="h-3" />,
              EmptyPlaceholder: () => (
                <div className="grid h-full place-items-center text-sm text-muted-foreground">
                  {COPY.noMessages}
                </div>
              ),
            }}
          />

          <AnimatePresence>
            {!atBottom ? (
              <motion.div
                initial={{ opacity: 0, y: 12 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: 12 }}
                className="absolute right-4 bottom-4"
              >
                <Button
                  type="button"
                  size="icon"
                  className="rounded-full shadow-md"
                  aria-label={COPY.jumpToBottom}
                  onClick={jumpBottom}
                >
                  <ChevronDown />
                </Button>
                {newCount > 0 ? (
                  <Badge
                    className="absolute top-1.5 right-12 cursor-pointer"
                    onClick={jumpBottom}
                  >
                    {newCountLabel(newCount)}
                  </Badge>
                ) : null}
              </motion.div>
            ) : null}
          </AnimatePresence>
        </div>

        {gated ? (
          <div className="border-t border-border bg-background px-4 py-6">
            <div className="mx-auto flex max-w-sm flex-col items-center gap-2 text-center">
              <UserAvatar name={active.title} size="lg" />
              <p className="text-sm font-medium">{COPY.notFriends}</p>
              <p className="text-xs text-muted-foreground">
                {isOutgoing(active.id)
                  ? COPY.waitingAccept
                  : isIncoming(active.id)
                    ? COPY.friendRequestToast
                    : COPY.addFriendToChat}
              </p>
              {isOutgoing(active.id) ? (
                <p className="text-xs text-muted-foreground">{COPY.requested}</p>
              ) : isIncoming(active.id) ? (
                <Button
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
                  onClick={() => {
                    void requestFriend(active.id).catch((err) =>
                      toast.error(err instanceof Error ? err.message : COPY.sendFailed),
                    );
                  }}
                >
                  <UserPlus />
                  {COPY.addFriend}
                </Button>
              )}
            </div>
          </div>
        ) : (
          <form
            onSubmit={onSubmit}
            onPaste={onPaste}
            className="flex items-end gap-2 border-t border-border bg-background px-3 pt-3 pb-[max(12px,env(safe-area-inset-bottom))]"
          >
            <input
              ref={fileRef}
              type="file"
              accept={ACCEPT_IMAGE}
              hidden
              onChange={(ev) => {
                void onPickImage(ev);
              }}
            />
            <InputGroup className="h-auto min-h-10 flex-1 items-end">
              <InputGroupAddon align="inline-start" className="self-end pb-1">
                <InputGroupButton
                  size="icon-sm"
                  aria-label={COPY.pickImage}
                  disabled={uploading || sending || status !== "online"}
                  onClick={() => fileRef.current?.click()}
                >
                  {uploading ? <Spinner /> : <ImageIcon />}
                </InputGroupButton>
              </InputGroupAddon>
              <InputGroupTextarea
                ref={inputRef}
                rows={1}
                value={draft}
                onChange={(e) => setDraft(e.target.value)}
                onKeyDown={onKey}
                placeholder={COPY.messagePlaceholder}
                maxLength={4000}
                aria-label={COPY.messagePlaceholder}
                className="max-h-40 min-h-10 py-2.5"
              />
              <InputGroupAddon align="inline-end" className="self-end pb-1">
                <Button
                  type="submit"
                  size="sm"
                  disabled={!draft.trim() || sending || status !== "online"}
                  aria-label={COPY.send}
                >
                  {sending ? <Spinner /> : <Send />}
                  {COPY.send}
                </Button>
              </InputGroupAddon>
            </InputGroup>
          </form>
        )}
      </div>

      {active.kind === "group" && membersOpen ? (
        <aside className="absolute top-0 right-0 bottom-0 z-2 flex w-[min(240px,80vw)] flex-col border-l border-border bg-background md:relative md:w-60">
          <div className="flex items-center justify-between border-b border-border px-4 py-3">
            <p className="text-sm font-medium">{COPY.members}</p>
            <GhostIconButton label={COPY.close} onClick={toggleMembers} className="md:hidden">
              <ArrowLeft />
            </GhostIconButton>
          </div>
          <ScrollArea className="min-h-0 flex-1">
            <div className="p-2">
              {members.map((name) => (
                <div key={name} className="flex items-center gap-2 px-2 py-1.5">
                  <UserAvatar name={name} />
                  <p className="truncate text-sm">
                    {name}
                    {name === account ? (
                      <span className="ml-1.5 text-xs text-muted-foreground">{COPY.you}</span>
                    ) : null}
                  </p>
                </div>
              ))}
            </div>
          </ScrollArea>
        </aside>
      ) : null}

      <ImageViewer src={viewer} open={Boolean(viewer)} onClose={() => setViewer(null)} />

      <AnimatePresence>
        {dragging ? (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            className="absolute inset-0 z-5 grid place-items-center bg-foreground/45 font-semibold text-background"
          >
            {COPY.dropImage}
          </motion.div>
        ) : null}
      </AnimatePresence>
    </div>
  );
}
