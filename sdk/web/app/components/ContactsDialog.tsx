import PersonAdd from "@mui/icons-material/PersonAdd";
import Search from "@mui/icons-material/Search";
import Group from "@mui/icons-material/Group";
import Badge from "@mui/material/Badge";
import Box from "@mui/material/Box";
import Button from "@mui/material/Button";
import InputAdornment from "@mui/material/InputAdornment";
import List from "@mui/material/List";
import ListItem from "@mui/material/ListItem";
import ListItemButton from "@mui/material/ListItemButton";
import ListItemText from "@mui/material/ListItemText";
import Stack from "@mui/material/Stack";
import Tab from "@mui/material/Tab";
import Tabs from "@mui/material/Tabs";
import TextField from "@mui/material/TextField";
import Typography from "@mui/material/Typography";
import { useEffect, useState, type FormEvent } from "react";
import { toast } from "sonner";

import { COPY } from "../copy.ts";
import { useChat, type Person } from "../state/ChatProvider.tsx";
import { Modal, UserAvatar } from "./ui.tsx";

export function ContactsDialog({
  open,
  onOpenChange,
  initialTab = "friends",
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  initialTab?: "friends" | "add" | "incoming";
}) {
  const {
    people,
    incomingPeople,
    outgoing,
    isFriend,
    searchUsers,
    requestFriend,
    acceptFriend,
    rejectFriend,
    openThread,
    refreshSocial,
    incomingCount,
  } = useChat();
  const [tab, setTab] = useState<"friends" | "add" | "incoming">(initialTab);
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<Person[]>([]);
  const [searched, setSearched] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);

  useEffect(() => {
    if (!open) {
      return;
    }
    setTab(initialTab);
    void refreshSocial();
  }, [open, initialTab, refreshSocial]);

  async function onSearch(ev: FormEvent) {
    ev.preventDefault();
    const q = query.trim();
    if (!q) {
      return;
    }
    setHits(await searchUsers(q));
    setSearched(true);
  }

  function openChat(p: Person) {
    openThread(p.account, "user", p.nickname);
    onOpenChange(false);
  }

  async function onRequest(account: string) {
    setBusy(account);
    try {
      await requestFriend(account);
    } catch (err) {
      toast.error(err instanceof Error ? err.message : COPY.sendFailed);
    } finally {
      setBusy(null);
    }
  }

  return (
    <Modal open={open} onOpenChange={onOpenChange} title={COPY.contacts} wide>
      <Tabs
        value={tab}
        onChange={(_, v: "friends" | "add" | "incoming") => setTab(v)}
        variant="fullWidth"
        sx={{ mb: 1.5 }}
      >
        <Tab value="friends" label={COPY.contacts} />
        <Tab value="add" label={COPY.addFriend} />
        <Tab
          value="incoming"
          label={
            <Badge color="primary" badgeContent={incomingCount} max={99}>
              {COPY.incoming}
            </Badge>
          }
        />
      </Tabs>

      {tab === "friends" ? (
        <List dense sx={{ maxHeight: "min(52vh, 420px)", overflowY: "auto" }}>
          {people.length === 0 ? (
            <Stack sx={{ py: 6, textAlign: "center", px: 2, alignItems: "center" }}>
              <Group color="primary" />
              <Typography variant="subtitle2" sx={{ mt: 1.5 }}>
                {COPY.noFriends}
              </Typography>
              <Typography variant="caption" color="text.secondary">
                {COPY.noFriendsHint}
              </Typography>
              <Button sx={{ mt: 2 }} startIcon={<PersonAdd />} onClick={() => setTab("add")}>
                {COPY.addFriend}
              </Button>
            </Stack>
          ) : (
            people.map((p) => (
              <ListItemButton key={p.account} onClick={() => openChat(p)}>
                <Box sx={{ mr: 1.5 }}>
                  <UserAvatar name={p.nickname} size={36} />
                </Box>
                <ListItemText primary={p.nickname} secondary={`@${p.account}`} />
                <Typography variant="caption" color="primary">
                  {COPY.chatAction}
                </Typography>
              </ListItemButton>
            ))
          )}
        </List>
      ) : null}

      {tab === "add" ? (
        <Stack component="form" spacing={1.5} onSubmit={(e) => void onSearch(e)}>
          <TextField
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              setSearched(false);
            }}
            placeholder={COPY.searchPeople}
            autoFocus
            fullWidth
            slotProps={{
              input: {
                startAdornment: (
                  <InputAdornment position="start">
                    <Search fontSize="small" />
                  </InputAdornment>
                ),
              },
            }}
          />
          <Button type="submit" variant="contained">
            {COPY.searchPeople}
          </Button>
          <List dense sx={{ maxHeight: 224, overflowY: "auto" }}>
            {searched && hits.length === 0 ? (
              <Typography variant="body2" color="text.secondary" sx={{ py: 4, textAlign: "center" }}>
                {COPY.searchEmpty}
              </Typography>
            ) : (
              hits.map((p) => {
                const friend = isFriend(p.account);
                const pending = outgoing.includes(p.account);
                return (
                  <ListItem key={p.account} disableGutters sx={{ px: 0.5 }}>
                    <Box sx={{ mr: 1.5 }}>
                      <UserAvatar name={p.nickname} size={36} />
                    </Box>
                    <ListItemText primary={p.nickname} secondary={`@${p.account}`} />
                    {friend ? (
                      <Button size="small" onClick={() => openChat(p)}>
                        {COPY.chatAction}
                      </Button>
                    ) : pending ? (
                      <Typography variant="caption" color="text.secondary">
                        {COPY.requested}
                      </Typography>
                    ) : (
                      <Button
                        size="small"
                        variant="contained"
                        disabled={busy === p.account}
                        onClick={() => void onRequest(p.account)}
                      >
                        {COPY.addFriend}
                      </Button>
                    )}
                  </ListItem>
                );
              })
            )}
          </List>
        </Stack>
      ) : null}

      {tab === "incoming" ? (
        <List dense sx={{ maxHeight: "min(52vh, 420px)", overflowY: "auto" }}>
          {incomingPeople.length === 0 ? (
            <Typography variant="body2" color="text.secondary" sx={{ py: 6, textAlign: "center" }}>
              {COPY.noIncoming}
            </Typography>
          ) : (
            incomingPeople.map((p) => (
              <ListItem key={p.account} disableGutters sx={{ px: 0.5 }}>
                <Box sx={{ mr: 1.5 }}>
                  <UserAvatar name={p.nickname} size={36} />
                </Box>
                <ListItemText primary={p.nickname} secondary={`@${p.account}`} />
                <Button
                  size="small"
                  onClick={() => {
                    void rejectFriend(p.account).catch((err) =>
                      toast.error(err instanceof Error ? err.message : COPY.sendFailed),
                    );
                  }}
                >
                  {COPY.reject}
                </Button>
                <Button
                  size="small"
                  variant="contained"
                  onClick={() => {
                    void acceptFriend(p.account).then(() => onOpenChange(false));
                  }}
                >
                  {COPY.accept}
                </Button>
              </ListItem>
            ))
          )}
        </List>
      ) : null}
    </Modal>
  );
}
