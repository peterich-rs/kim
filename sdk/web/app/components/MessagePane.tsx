import ArrowBack from "@mui/icons-material/ArrowBack";
import ErrorIcon from "@mui/icons-material/Error";
import ImageOutlined from "@mui/icons-material/ImageOutlined";
import KeyboardArrowDown from "@mui/icons-material/KeyboardArrowDown";
import PersonAdd from "@mui/icons-material/PersonAdd";
import Send from "@mui/icons-material/Send";
import Chat from "@mui/icons-material/Chat";
import People from "@mui/icons-material/People";
import Box from "@mui/material/Box";
import Button from "@mui/material/Button";
import Chip from "@mui/material/Chip";
import CircularProgress from "@mui/material/CircularProgress";
import Fab from "@mui/material/Fab";
import IconButton from "@mui/material/IconButton";
import Stack from "@mui/material/Stack";
import TextField from "@mui/material/TextField";
import Typography from "@mui/material/Typography";
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
import { useChat, type ChatMsg } from "../state/ChatProvider.tsx";
import { ImageBubble } from "./ImageBubble.tsx";
import { ImageViewer } from "./ImageViewer.tsx";
import { IconTip, UserAvatar } from "./ui.tsx";

const GROUP_MS = 5 * 60 * 1000;
const START_INDEX = 100_000;

function sameGroup(prev: ChatMsg | undefined, cur: ChatMsg): boolean {
  if (!prev || prev.sys || cur.sys) {
    return false;
  }
  if (startOfDay(new Date(prev.at)) !== startOfDay(new Date(cur.at))) {
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
  const [headerDate, setHeaderDate] = useState("");
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

  function onKey(ev: KeyboardEvent<HTMLDivElement>) {
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
      <Box
        sx={{
          display: { xs: "none", md: "flex" },
          flex: 1,
          minHeight: 0,
          alignItems: "center",
          justifyContent: "center",
          flexDirection: "column",
          bgcolor: (theme) => theme.palette.canvas,
          textAlign: "center",
          px: 3,
        }}
      >
        <Box
          sx={{
            width: 64,
            height: 64,
            borderRadius: 3,
            bgcolor: "background.paper",
            display: "grid",
            placeItems: "center",
            color: "primary.main",
            mb: 2,
          }}
        >
          <Chat />
        </Box>
        <Typography variant="h6">{COPY.pickConversation}</Typography>
        <Typography variant="body2" color="text.secondary" sx={{ mt: 0.75, maxWidth: 280 }}>
          {COPY.pickConversationHint}
        </Typography>
      </Box>
    );
  }

  const subtitle = active.kind === "group" ? COPY.groupChat : COPY.privateChat;
  const gated = active.kind === "user" && socialReady && !isFriend(active.id);

  return (
    <Box
      sx={{ position: "relative", display: "flex", flex: 1, minWidth: 0, minHeight: 0 }}
      onDragOver={(ev) => {
        ev.preventDefault();
        setDragging(true);
      }}
      onDragLeave={() => setDragging(false)}
      onDrop={onDrop}
    >
      <Box sx={{ display: "flex", flexDirection: "column", flex: 1, minWidth: 0, bgcolor: (theme) => theme.palette.canvas }}>
        <Stack direction="row" spacing={1.25} sx={{ px: 1.25,
            py: 1,
            bgcolor: "background.paper",
            borderBottom: 1,
            borderColor: "divider",
            minHeight: 56, alignItems: "center" }}>
          <IconButton aria-label={COPY.back} onClick={closeThread} sx={{ display: { md: "none" } }}>
            <ArrowBack />
          </IconButton>
          <UserAvatar name={active.title} size={36} />
          <Box sx={{ minWidth: 0, flex: 1 }}>
            <Typography noWrap variant="subtitle2">
              {active.title}
            </Typography>
            <Typography variant="caption" color="text.secondary">
              {active.kind === "group" && members.length > 0 ? memberCount(members.length) : subtitle}
            </Typography>
          </Box>
          {active.kind === "group" ? (
            <IconTip label={COPY.members}>
              <IconButton aria-label={COPY.members} onClick={toggleMembers}>
                <People />
              </IconButton>
            </IconTip>
          ) : null}
        </Stack>

        <Box sx={{ position: "relative", flex: 1, minHeight: 0 }}>
          {headerDate ? (
            <Chip
              size="small"
              label={headerDate}
              sx={{
                position: "absolute",
                top: 8,
                left: "50%",
                transform: "translateX(-50%)",
                zIndex: 2,
                bgcolor: "background.paper",
                boxShadow: 1,
              }}
            />
          ) : null}
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
            rangeChanged={(range) => {
              const idx =
                range.startIndex >= firstItemIndex
                  ? range.startIndex - firstItemIndex
                  : range.startIndex;
              const item = grouped[idx] ?? grouped[0];
              const row = item?.row;
              if (row) {
                setHeaderDate(formatDateSep(row.at));
              }
            }}
            increaseViewportBy={200}
            itemContent={(_index, item) => {
              const { row, cont, showDate } = item;
              if (row.sys) {
                return (
                  <Box sx={{ py: 1, textAlign: "center" }}>
                    <Typography variant="caption" color="text.secondary">
                      {row.body}
                    </Typography>
                  </Box>
                );
              }
              const mine = row.sender === account;
              return (
                <Box sx={{ px: 2, pt: showDate ? 1.5 : cont ? 0.25 : 1.25 }}>
                  {showDate ? (
                    <Box sx={{ display: "flex", justifyContent: "center", mb: 1.25 }}>
                      <Chip size="small" label={formatDateSep(row.at)} />
                    </Box>
                  ) : null}
                  <Stack
                    direction={mine ? "row-reverse" : "row"}
                    spacing={1}
                    sx={{ maxWidth: "min(72%, 36rem)", ml: mine ? "auto" : 0, mr: mine ? 0 : "auto" }}
                  >
                    {cont ? (
                      <Box sx={{ width: 32, flexShrink: 0 }} />
                    ) : (
                      <UserAvatar name={row.sender} size={32} />
                    )}
                    <Box sx={{ minWidth: 0 }}>
                      {cont ? null : (
                        <Stack direction={mine ? "row-reverse" : "row"} spacing={1} sx={{ mb: 0.4, alignItems: "baseline" }}>
                          <Typography variant="caption" color="text.secondary" sx={{ fontWeight: 600 }}>
                            {mine ? COPY.you : row.sender}
                          </Typography>
                          <Typography variant="caption" color="text.secondary">
                            {formatClock(row.at)}
                          </Typography>
                        </Stack>
                      )}
                      {isImageMessage(row.type, row.body, row.extra) ? (
                        <ImageBubble
                          src={row.body}
                          size={row.width > 0 && row.height > 0 ? { w: row.width, h: row.height } : undefined}
                          mine={mine}
                          onOpen={setViewer}
                        />
                      ) : (
                        <Box
                          sx={{
                            px: 1.5,
                            py: 1,
                            borderRadius: 2,
                            borderTopRightRadius: mine ? 4 : 16,
                            borderTopLeftRadius: mine ? 16 : 4,
                            bgcolor: (theme) => (mine ? theme.palette.bubbleMe : theme.palette.bubbleThem),
                            color: "text.primary",
                            boxShadow: mine ? "none" : 1,
                            whiteSpace: "pre-wrap",
                            wordBreak: "break-word",
                            fontSize: 14,
                            lineHeight: 1.5,
                            opacity: row.status === "failed" ? 0.85 : 1,
                          }}
                        >
                          {row.body}
                        </Box>
                      )}
                      {row.status === "sending" ? (
                        <Typography variant="caption" color="text.secondary">
                          {COPY.sending}
                        </Typography>
                      ) : null}
                      {row.status === "failed" ? (
                        <Button
                          size="small"
                          color="error"
                          startIcon={<ErrorIcon fontSize="small" />}
                          onClick={() => {
                            void retryMessage(row.key).catch((err) =>
                              toast.error(err instanceof Error ? err.message : COPY.sendFailed),
                            );
                          }}
                          sx={{ mt: 0.25, minHeight: 28 }}
                        >
                          {COPY.retrySend}
                        </Button>
                      ) : null}
                    </Box>
                  </Stack>
                </Box>
              );
            }}
            components={{
              EmptyPlaceholder: () => (
                <Box sx={{ height: "100%", display: "grid", placeItems: "center" }}>
                  <Typography variant="body2" color="text.secondary">
                    {COPY.noMessages}
                  </Typography>
                </Box>
              ),
            }}
          />

          <AnimatePresence>
            {!atBottom ? (
              <motion.div
                initial={{ opacity: 0, y: 12 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: 12 }}
                style={{ position: "absolute", right: 16, bottom: 16 }}
              >
                <Fab
                  size="small"
                  color="primary"
                  aria-label={COPY.jumpToBottom}
                  onClick={jumpBottom}
                >
                  <KeyboardArrowDown />
                </Fab>
                {newCount > 0 ? (
                  <Chip
                    size="small"
                    color="primary"
                    label={newCountLabel(newCount)}
                    onClick={jumpBottom}
                    sx={{ position: "absolute", right: 48, top: 6, cursor: "pointer" }}
                  />
                ) : null}
              </motion.div>
            ) : null}
          </AnimatePresence>
        </Box>

        {gated ? (
          <Box sx={{ bgcolor: "background.paper", borderTop: 1, borderColor: "divider", px: 2, py: 2.5 }}>
            <Stack spacing={1} sx={{ maxWidth: 360, mx: "auto", textAlign: "center", alignItems: "center" }}>
              <UserAvatar name={active.title} size={48} />
              <Typography variant="subtitle2">{COPY.notFriends}</Typography>
              <Typography variant="caption" color="text.secondary">
                {isOutgoing(active.id)
                  ? COPY.waitingAccept
                  : isIncoming(active.id)
                    ? COPY.friendRequestToast
                    : COPY.addFriendToChat}
              </Typography>
              {isOutgoing(active.id) ? (
                <Typography variant="caption" color="text.secondary">
                  {COPY.requested}
                </Typography>
              ) : isIncoming(active.id) ? (
                <Button
                  variant="contained"
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
                  variant="contained"
                  startIcon={<PersonAdd />}
                  onClick={() => {
                    void requestFriend(active.id).catch((err) =>
                      toast.error(err instanceof Error ? err.message : COPY.sendFailed),
                    );
                  }}
                >
                  {COPY.addFriend}
                </Button>
              )}
            </Stack>
          </Box>
        ) : (
          <Box
            component="form"
            onSubmit={onSubmit}
            onPaste={onPaste}
            sx={{
              display: "flex",
              alignItems: "flex-end",
              gap: 1,
              px: 1.25,
              py: 1.25,
              pb: "max(12px, env(safe-area-inset-bottom))",
              bgcolor: "background.paper",
              borderTop: 1,
              borderColor: "divider",
            }}
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
            <IconTip label={COPY.pickImage}>
              <span>
                <IconButton
                  aria-label={COPY.pickImage}
                  disabled={uploading || sending || status !== "online"}
                  onClick={() => fileRef.current?.click()}
                >
                  {uploading ? <CircularProgress size={18} /> : <ImageOutlined />}
                </IconButton>
              </span>
            </IconTip>
            <TextField
              inputRef={inputRef}
              fullWidth
              multiline
              minRows={1}
              maxRows={6}
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              onKeyDown={onKey}
              placeholder={COPY.messagePlaceholder}
              slotProps={{ htmlInput: { maxLength: 4000, "aria-label": COPY.messagePlaceholder } }}
            />
            <motion.div whileTap={{ scale: 0.94 }}>
              <Button
                type="submit"
                variant="contained"
                disabled={!draft.trim() || sending || status !== "online"}
                aria-label={COPY.send}
                sx={{ minWidth: 64, height: 40 }}
                endIcon={sending ? <CircularProgress size={14} color="inherit" /> : <Send />}
              >
                {COPY.send}
              </Button>
            </motion.div>
          </Box>
        )}
      </Box>

      {active.kind === "group" && membersOpen ? (
        <Box
          sx={{
            width: { xs: "min(240px, 80vw)", md: 240 },
            position: { xs: "absolute", md: "relative" },
            right: 0,
            top: 0,
            bottom: 0,
            zIndex: 2,
            bgcolor: "background.paper",
            borderLeft: 1,
            borderColor: "divider",
            display: "flex",
            flexDirection: "column",
          }}
        >
          <Stack direction="row" sx={{ px: 2, py: 1.5, borderBottom: 1, borderColor: "divider", alignItems: "center", justifyContent: "space-between" }}>
            <Typography variant="subtitle2">{COPY.members}</Typography>
            <IconButton aria-label={COPY.close} onClick={toggleMembers} sx={{ display: { md: "none" } }}>
              <ArrowBack fontSize="small" />
            </IconButton>
          </Stack>
          <Box sx={{ flex: 1, overflowY: "auto", p: 1 }}>
            {members.map((name) => (
              <Stack key={name} direction="row" spacing={1} sx={{ px: 1, py: 0.75, alignItems: "center" }}>
                <UserAvatar name={name} size={32} />
                <Typography noWrap variant="body2">
                  {name}
                  {name === account ? (
                    <Typography component="span" variant="caption" color="text.secondary" sx={{ ml: 0.75 }}>
                      {COPY.you}
                    </Typography>
                  ) : null}
                </Typography>
              </Stack>
            ))}
          </Box>
        </Box>
      ) : null}

      <ImageViewer src={viewer} open={Boolean(viewer)} onClose={() => setViewer(null)} />

      <AnimatePresence>
        {dragging ? (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            style={{
              position: "absolute",
              inset: 0,
              zIndex: 5,
              display: "grid",
              placeItems: "center",
              background: "rgba(15,23,42,0.45)",
              color: "#fff",
              fontWeight: 600,
            }}
          >
            {COPY.dropImage}
          </motion.div>
        ) : null}
      </AnimatePresence>
    </Box>
  );
}
