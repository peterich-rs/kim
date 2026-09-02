import Add from "@mui/icons-material/Add";
import Group from "@mui/icons-material/Group";
import Logout from "@mui/icons-material/Logout";
import PersonAdd from "@mui/icons-material/PersonAdd";
import Search from "@mui/icons-material/Search";
import Tag from "@mui/icons-material/Tag";
import VolumeOff from "@mui/icons-material/VolumeOff";
import Badge from "@mui/material/Badge";
import Box from "@mui/material/Box";
import IconButton from "@mui/material/IconButton";
import InputAdornment from "@mui/material/InputAdornment";
import List from "@mui/material/List";
import ListItemButton from "@mui/material/ListItemButton";
import ListItemText from "@mui/material/ListItemText";
import Menu from "@mui/material/Menu";
import MenuItem from "@mui/material/MenuItem";
import Skeleton from "@mui/material/Skeleton";
import Stack from "@mui/material/Stack";
import TextField from "@mui/material/TextField";
import Typography from "@mui/material/Typography";
import { useEffect, useMemo, useState } from "react";
import { useDebounceValue } from "usehooks-ts";

import { COPY } from "../copy.ts";
import { formatListTime } from "../lib/format.ts";
import { useChat } from "../state/ChatProvider.tsx";
import { UserAvatar } from "./ui.tsx";

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
  const [menu, setMenu] = useState<{ id: string; muted: boolean; anchor: HTMLElement } | null>(null);
  const [focusIndex, setFocusIndex] = useState(0);

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
        setFocusIndex((i) => Math.min(filtered.length - 1, i + 1));
      } else if (ev.key === "k" || ev.key === "ArrowUp") {
        ev.preventDefault();
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
    <Box
      component="aside"
      sx={{
        display: "flex",
        flexDirection: "column",
        height: "100%",
        minHeight: 0,
        width: { xs: "100%", md: 340 },
        bgcolor: "background.paper",
        borderRight: { md: 1 },
        borderColor: { md: "divider" },
      }}
    >
      <Stack direction="row" sx={{ px: 2, pt: 1.75, pb: 1, alignItems: "center", justifyContent: "space-between" }}>
        <Typography variant="h6" sx={{ fontWeight: 700, letterSpacing: "-0.02em" }}>
          {COPY.conversations}
        </Typography>
        <Stack direction="row" spacing={0.25}>
          <IconButton aria-label={COPY.contacts} onClick={onNewChat} size="small">
            <Badge color="primary" badgeContent={incomingCount} max={9} overlap="circular">
              <Group fontSize="small" />
            </Badge>
          </IconButton>
          <IconButton aria-label={COPY.startChat} onClick={onAddFriend} size="small">
            <PersonAdd fontSize="small" />
          </IconButton>
          <IconButton aria-label={COPY.newGroup} onClick={onNewGroup} size="small">
            <Add fontSize="small" />
          </IconButton>
        </Stack>
      </Stack>

      <Box sx={{ px: 1.5, pb: 1 }}>
        <TextField
          fullWidth
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder={COPY.searchPlaceholder}
          slotProps={{
            htmlInput: { "aria-label": COPY.searchPlaceholder },
            input: {
              startAdornment: (
                <InputAdornment position="start">
                  <Search fontSize="small" />
                </InputAdornment>
              ),
            },
          }}
        />
      </Box>

      <List dense disablePadding sx={{ flex: 1, minHeight: 0, overflowY: "auto", px: 0.75 }}>
        {showSkeleton
          ? Array.from({ length: 8 }, (_, i) => (
              <Stack key={i} direction="row" spacing={1.5} sx={{ px: 1, py: 1, height: 60, alignItems: "center" }}>
                <Skeleton variant="rounded" width={40} height={40} />
                <Box sx={{ flex: 1 }}>
                  <Skeleton width="50%" />
                  <Skeleton width="80%" />
                </Box>
              </Stack>
            ))
          : filtered.length === 0
            ? (
              <Box sx={{ px: 2, py: 8, textAlign: "center" }}>
                <Typography variant="subtitle2" color="text.primary">
                  {threads.length === 0 ? COPY.noConversations : COPY.noMatch}
                </Typography>
                {threads.length === 0 ? (
                  <>
                    <Typography variant="body2" color="text.secondary" sx={{ mt: 0.75 }}>
                      {COPY.noConversationsHint}
                    </Typography>
                    <Box
                      component="button"
                      type="button"
                      onClick={onAddFriend}
                      sx={{
                        mt: 2,
                        border: 0,
                        bgcolor: "transparent",
                        color: "primary.main",
                        cursor: "pointer",
                        fontWeight: 600,
                      }}
                    >
                      {COPY.addFriend}
                    </Box>
                  </>
                ) : null}
              </Box>
            )
            : filtered.map((t, i) => (
                <ListItemButton
                  key={t.id}
                  selected={t.id === activeId || i === focusIndex}
                  onClick={() => openThread(t.id, t.kind, t.title)}
                  onContextMenu={(ev) => {
                    ev.preventDefault();
                    setMenu({ id: t.id, muted: t.muted, anchor: ev.currentTarget });
                  }}
                  sx={{
                    height: 60,
                    mb: 0.25,
                    opacity: t.muted ? 0.72 : 1,
                    alignItems: "center",
                    gap: 1.25,
                  }}
                >
                  <Box sx={{ position: "relative" }}>
                    <UserAvatar name={t.title} size={40} />
                    {t.kind === "group" ? (
                      <Box
                        sx={{
                          position: "absolute",
                          right: -2,
                          bottom: -2,
                          width: 16,
                          height: 16,
                          borderRadius: 0.75,
                          bgcolor: "background.paper",
                          display: "grid",
                          placeItems: "center",
                          color: "text.secondary",
                        }}
                      >
                        <Tag sx={{ fontSize: 12 }} />
                      </Box>
                    ) : null}
                  </Box>
                  <ListItemText
                    primary={
                      <Stack direction="row" sx={{ justifyContent: "space-between", gap: 1, alignItems: "baseline" }}>
                        <Typography noWrap variant="body2" sx={{ fontWeight: t.unread > 0 ? 700 : 600 }}>
                          {t.title}
                        </Typography>
                        {t.lastAt ? (
                          <Typography variant="caption" color="text.secondary" sx={{ flexShrink: 0 }}>
                            {formatListTime(t.lastAt)}
                          </Typography>
                        ) : null}
                      </Stack>
                    }
                    secondary={
                      <Stack direction="row" sx={{ alignItems: "center", gap: 0.75 }}>
                        {t.muted ? <VolumeOff sx={{ fontSize: 14, color: "text.secondary" }} /> : null}
                        <Typography noWrap variant="caption" color="text.secondary" sx={{ flex: 1 }}>
                          {t.lastBody || COPY.noMessages}
                        </Typography>
                        {t.unread > 0 ? (
                          <Box
                            aria-label={`${t.unread}`}
                            sx={{
                              ml: "auto",
                              minWidth: 20,
                              height: 20,
                              px: 0.75,
                              borderRadius: 999,
                              fontSize: 11,
                              fontWeight: 700,
                              display: "grid",
                              placeItems: "center",
                              bgcolor: (theme) => theme.palette.unread,
                              color: "#fff",
                            }}
                          >
                            {t.unread > 99 ? "99+" : t.unread}
                          </Box>
                        ) : null}
                      </Stack>
                    }
                    slotProps={{ secondary: { component: "div" } }}
                  />
                </ListItemButton>
              ))}
      </List>

      <Menu
        open={Boolean(menu)}
        anchorEl={menu?.anchor}
        onClose={() => setMenu(null)}
      >
        <MenuItem
          onClick={() => {
            if (menu) {
              muteThread(menu.id, !menu.muted);
            }
            setMenu(null);
          }}
        >
          {menu?.muted ? COPY.unmuteChat : COPY.muteChat}
        </MenuItem>
      </Menu>

      <Stack direction="row" spacing={1.25} sx={{ px: 1.5, py: 1.25, borderTop: 1, borderColor: "divider", alignItems: "center" }}>
        <IconButton aria-label={COPY.profile} onClick={onProfile} size="small">
          <UserAvatar name={nickname || account || "?"} size={36} />
        </IconButton>
        <Box sx={{ minWidth: 0, flex: 1 }}>
          <Typography noWrap variant="body2" sx={{ fontWeight: 600 }}>
            {nickname || account}
          </Typography>
          <Typography variant="caption" color="text.secondary">
            {statusLabel}
          </Typography>
        </Box>
        <IconButton aria-label={COPY.logout} onClick={() => void signOut()} size="small">
          <Logout fontSize="small" />
        </IconButton>
      </Stack>
    </Box>
  );
}
