import { useState, type FormEvent } from "react";

import { COPY } from "../copy.ts";
import { validateAccount } from "../lib/validation.ts";
import { useChat } from "../state/ChatProvider.tsx";
import { Button, Field, Modal, TextInput } from "./ui.tsx";

export function NewChatDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const { account, openThread } = useChat();
  const [peer, setPeer] = useState("");
  const [error, setError] = useState("");

  function onSubmit(ev: FormEvent) {
    ev.preventDefault();
    const dest = peer.trim();
    const invalid = validateAccount(dest);
    if (invalid) {
      setError(invalid);
      return;
    }
    if (dest === account) {
      setError(COPY.cannotChatSelf);
      return;
    }
    openThread(dest, "user", dest);
    setPeer("");
    setError("");
    onOpenChange(false);
  }

  return (
    <Modal open={open} onOpenChange={onOpenChange} title={COPY.newChat}>
      <form className="flex flex-col gap-4" onSubmit={onSubmit}>
        <Field label={COPY.peerAccount} error={error}>
          <TextInput
            value={peer}
            onChange={(e) => {
              setPeer(e.target.value);
              setError("");
            }}
            placeholder={COPY.peerPlaceholder}
            spellCheck={false}
            maxLength={32}
            autoFocus
          />
        </Field>
        <div className="flex justify-end gap-2">
          <Button type="button" variant="ghost" onClick={() => onOpenChange(false)}>
            {COPY.cancel}
          </Button>
          <Button type="submit">{COPY.openChat}</Button>
        </div>
      </form>
    </Modal>
  );
}
