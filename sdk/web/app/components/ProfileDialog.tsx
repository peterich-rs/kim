import Box from "@mui/material/Box";
import Button from "@mui/material/Button";
import Stack from "@mui/material/Stack";
import TextField from "@mui/material/TextField";
import ToggleButton from "@mui/material/ToggleButton";
import ToggleButtonGroup from "@mui/material/ToggleButtonGroup";
import Typography from "@mui/material/Typography";
import { useColorScheme } from "@mui/material/styles";
import { useEffect, useState, type FormEvent } from "react";
import { toast } from "sonner";

import { COPY } from "../copy.ts";
import { useChat } from "../state/ChatProvider.tsx";
import { Modal } from "./ui.tsx";

export function ProfileDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const { account, nickname, updateProfile, changePassword } = useChat();
  const { mode, setMode } = useColorScheme();
  const [name, setName] = useState(nickname);
  const [bio, setBio] = useState("");
  const [oldPassword, setOldPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (open) {
      setName(nickname);
    }
  }, [open, nickname]);

  async function onSave(ev: FormEvent) {
    ev.preventDefault();
    setBusy(true);
    try {
      await updateProfile(name.trim(), bio.trim());
      if (oldPassword && newPassword) {
        await changePassword(oldPassword, newPassword);
        setOldPassword("");
        setNewPassword("");
      }
    } catch (err) {
      toast.error(err instanceof Error ? err.message : COPY.sendFailed);
    } finally {
      setBusy(false);
    }
  }

  return (
    <Modal open={open} onOpenChange={onOpenChange} title={COPY.profile}>
      <Stack component="form" spacing={2} onSubmit={(e) => void onSave(e)} sx={{ pt: 0.5 }}>
        <Typography variant="caption" color="text.secondary">
          @{account}
        </Typography>
        <Box>
          <Typography variant="caption" color="text.secondary" sx={{ display: "block", mb: 0.75 }}>
            {COPY.theme}
          </Typography>
          <ToggleButtonGroup
            exclusive
            fullWidth
            size="small"
            value={mode ?? "system"}
            onChange={(_, value: "light" | "dark" | "system" | null) => {
              if (value) {
                setMode(value);
              }
            }}
            aria-label={COPY.theme}
          >
            <ToggleButton value="system">{COPY.themeSystem}</ToggleButton>
            <ToggleButton value="light">{COPY.themeLight}</ToggleButton>
            <ToggleButton value="dark">{COPY.themeDark}</ToggleButton>
          </ToggleButtonGroup>
        </Box>
        <TextField
          label={COPY.nickname}
          value={name}
          onChange={(e) => setName(e.target.value)}
          slotProps={{ htmlInput: { maxLength: 32 } }}
          fullWidth
        />
        <TextField
          label={COPY.bio}
          value={bio}
          onChange={(e) => setBio(e.target.value)}
          slotProps={{ htmlInput: { maxLength: 160 } }}
          fullWidth
        />
        <TextField
          label={COPY.oldPassword}
          type="password"
          value={oldPassword}
          onChange={(e) => setOldPassword(e.target.value)}
          autoComplete="current-password"
          fullWidth
        />
        <TextField
          label={COPY.newPassword}
          type="password"
          value={newPassword}
          onChange={(e) => setNewPassword(e.target.value)}
          autoComplete="new-password"
          helperText={COPY.passwordHint}
          fullWidth
        />
        <Button type="submit" variant="contained" disabled={busy || !name.trim()}>
          {COPY.saveProfile}
        </Button>
      </Stack>
    </Modal>
  );
}
