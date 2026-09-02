import Check from "@mui/icons-material/Check";
import Box from "@mui/material/Box";
import Button from "@mui/material/Button";
import CircularProgress from "@mui/material/CircularProgress";
import List from "@mui/material/List";
import ListItemButton from "@mui/material/ListItemButton";
import ListItemText from "@mui/material/ListItemText";
import Stack from "@mui/material/Stack";
import TextField from "@mui/material/TextField";
import Typography from "@mui/material/Typography";
import { useState, type FormEvent } from "react";

import { COPY } from "../copy.ts";
import { mapUserError } from "../lib/errors.ts";
import { useChat } from "../state/ChatProvider.tsx";
import { Modal, UserAvatar } from "./ui.tsx";

export function NewGroupDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const { account, createGroup, people } = useChat();
  const [name, setName] = useState("");
  const [members, setMembers] = useState<string[]>([]);
  const [error, setError] = useState("");
  const [pending, setPending] = useState(false);

  function toggleMember(acc: string) {
    setMembers((prev) => (prev.includes(acc) ? prev.filter((x) => x !== acc) : [...prev, acc]));
    setError("");
  }

  async function onSubmit(ev: FormEvent) {
    ev.preventDefault();
    const title = name.trim();
    if (!title) {
      setError(COPY.required);
      return;
    }
    setPending(true);
    setError("");
    try {
      await createGroup(title, members);
      setName("");
      setMembers([]);
      onOpenChange(false);
    } catch (err) {
      setError(mapUserError(err));
    } finally {
      setPending(false);
    }
  }

  return (
    <Modal open={open} onOpenChange={onOpenChange} title={COPY.newGroup}>
      <Stack component="form" spacing={2} onSubmit={(ev) => void onSubmit(ev)}>
        <TextField
          label={COPY.groupName}
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder={COPY.groupNamePlaceholder}
          slotProps={{ htmlInput: { maxLength: 32 } }}
          autoFocus
          fullWidth
        />
        <Box>
          <Typography variant="caption" color="text.secondary">
            {COPY.pickFriends}
          </Typography>
          <List dense sx={{ maxHeight: 192, overflowY: "auto", border: 1, borderColor: "divider", borderRadius: 2, mt: 0.75 }}>
            {people.length === 0 ? (
              <Typography variant="caption" color="text.secondary" sx={{ display: "block", px: 2, py: 3, textAlign: "center" }}>
                {COPY.noFriendsHint}
              </Typography>
            ) : (
              people.map((p) => {
                const on = members.includes(p.account);
                return (
                  <ListItemButton key={p.account} selected={on} onClick={() => toggleMember(p.account)}>
                    <Box sx={{ mr: 1.25 }}>
                      <UserAvatar name={p.nickname} size={32} />
                    </Box>
                    <ListItemText primary={p.nickname} />
                    {on ? <Check fontSize="small" color="primary" /> : null}
                  </ListItemButton>
                );
              })
            )}
          </List>
          {account ? (
            <Typography variant="caption" color="text.secondary" sx={{ mt: 0.75, display: "block" }}>
              {account} · {COPY.you}
            </Typography>
          ) : null}
          {error ? (
            <Typography variant="caption" color="error" sx={{ mt: 0.5, display: "block" }}>
              {error}
            </Typography>
          ) : null}
        </Box>
        <Stack direction="row" spacing={1} sx={{ justifyContent: "flex-end" }}>
          <Button onClick={() => onOpenChange(false)}>{COPY.cancel}</Button>
          <Button type="submit" variant="contained" disabled={pending}>
            {pending ? <CircularProgress size={16} color="inherit" sx={{ mr: 1 }} /> : null}
            {pending ? COPY.creating : COPY.createGroup}
          </Button>
        </Stack>
      </Stack>
    </Modal>
  );
}
