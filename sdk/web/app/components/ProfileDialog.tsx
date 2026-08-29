import { useEffect, useState, type FormEvent } from "react";
import { toast } from "sonner";

import { COPY } from "../copy.ts";
import { useChat } from "../state/ChatProvider.tsx";
import { Button, Field, Modal, TextInput } from "./ui.tsx";

export function ProfileDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const { account, nickname, updateProfile, changePassword } = useChat();
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
      <form className="flex flex-col gap-4" onSubmit={(e) => void onSave(e)}>
        <p className="text-xs text-muted">@{account}</p>
        <Field label={COPY.nickname}>
          <TextInput value={name} onChange={(e) => setName(e.target.value)} maxLength={32} />
        </Field>
        <Field label={COPY.bio}>
          <TextInput value={bio} onChange={(e) => setBio(e.target.value)} maxLength={160} />
        </Field>
        <Field label={COPY.oldPassword}>
          <TextInput
            type="password"
            value={oldPassword}
            onChange={(e) => setOldPassword(e.target.value)}
            autoComplete="current-password"
          />
        </Field>
        <Field label={COPY.newPassword} hint={COPY.passwordHint}>
          <TextInput
            type="password"
            value={newPassword}
            onChange={(e) => setNewPassword(e.target.value)}
            autoComplete="new-password"
          />
        </Field>
        <Button type="submit" disabled={busy || !name.trim()}>
          {COPY.saveProfile}
        </Button>
      </form>
    </Modal>
  );
}
