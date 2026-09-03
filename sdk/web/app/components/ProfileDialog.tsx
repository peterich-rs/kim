import { useTheme } from "next-themes";
import { useEffect, useState, type FormEvent } from "react";
import { toast } from "sonner";

import { COPY } from "../copy.ts";
import { useChat } from "../state/ChatProvider.tsx";
import { Modal } from "./ui.tsx";
import { Button } from "./ui/button.tsx";
import { Field, FieldDescription, FieldLabel } from "./ui/field.tsx";
import { Input } from "./ui/input.tsx";
import { ToggleGroup, ToggleGroupItem } from "./ui/toggle-group.tsx";

export function ProfileDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const { account, nickname, updateProfile, changePassword } = useChat();
  const { theme, setTheme } = useTheme();
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
      <form className="flex flex-col gap-4 pt-1" onSubmit={(e) => void onSave(e)}>
        <p className="text-xs text-muted-foreground">@{account}</p>
        <Field>
          <FieldLabel>{COPY.theme}</FieldLabel>
          <ToggleGroup
            className="w-full"
            value={[theme ?? "system"]}
            onValueChange={(value) => {
              const next = value[0];
              if (next === "light" || next === "dark" || next === "system") {
                setTheme(next);
              }
            }}
            variant="outline"
          >
            <ToggleGroupItem value="system" className="flex-1">
              {COPY.themeSystem}
            </ToggleGroupItem>
            <ToggleGroupItem value="light" className="flex-1">
              {COPY.themeLight}
            </ToggleGroupItem>
            <ToggleGroupItem value="dark" className="flex-1">
              {COPY.themeDark}
            </ToggleGroupItem>
          </ToggleGroup>
        </Field>
        <Field>
          <FieldLabel htmlFor="nickname">{COPY.nickname}</FieldLabel>
          <Input
            id="nickname"
            value={name}
            onChange={(e) => setName(e.target.value)}
            maxLength={32}
          />
        </Field>
        <Field>
          <FieldLabel htmlFor="bio">{COPY.bio}</FieldLabel>
          <Input id="bio" value={bio} onChange={(e) => setBio(e.target.value)} maxLength={160} />
        </Field>
        <Field>
          <FieldLabel htmlFor="old-password">{COPY.oldPassword}</FieldLabel>
          <Input
            id="old-password"
            type="password"
            value={oldPassword}
            onChange={(e) => setOldPassword(e.target.value)}
            autoComplete="current-password"
          />
        </Field>
        <Field>
          <FieldLabel htmlFor="new-password">{COPY.newPassword}</FieldLabel>
          <Input
            id="new-password"
            type="password"
            value={newPassword}
            onChange={(e) => setNewPassword(e.target.value)}
            autoComplete="new-password"
          />
          <FieldDescription>{COPY.passwordHint}</FieldDescription>
        </Field>
        <Button type="submit" disabled={busy || !name.trim()}>
          {COPY.saveProfile}
        </Button>
      </form>
    </Modal>
  );
}
