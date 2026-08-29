import { Loader2, Plus, X } from "lucide-react";
import { useState, type FormEvent, type KeyboardEvent } from "react";

import { COPY } from "../copy.ts";
import { mapUserError } from "../lib/errors.ts";
import { validateAccount } from "../lib/validation.ts";
import { useChat } from "../state/ChatProvider.tsx";
import { Button, Field, Modal, TextInput } from "./ui.tsx";

export function NewGroupDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const { account, createGroup } = useChat();
  const [name, setName] = useState("");
  const [draft, setDraft] = useState("");
  const [members, setMembers] = useState<string[]>([]);
  const [error, setError] = useState("");
  const [pending, setPending] = useState(false);

  function addMember() {
    const acc = draft.trim();
    const invalid = validateAccount(acc);
    if (invalid) {
      setError(invalid);
      return;
    }
    if (acc === account || members.includes(acc)) {
      setDraft("");
      setError("");
      return;
    }
    setMembers((prev) => [...prev, acc]);
    setDraft("");
    setError("");
  }

  function onKey(ev: KeyboardEvent<HTMLInputElement>) {
    if (ev.key === "Enter") {
      ev.preventDefault();
      addMember();
    }
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
      setDraft("");
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
      <form className="flex flex-col gap-4" onSubmit={(ev) => void onSubmit(ev)}>
        <Field label={COPY.groupName}>
          <TextInput
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder={COPY.groupNamePlaceholder}
            maxLength={32}
            autoFocus
          />
        </Field>
        <Field label={COPY.groupMembers} error={error}>
          <div className="flex gap-2">
            <TextInput
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              onKeyDown={onKey}
              placeholder={COPY.memberPlaceholder}
              spellCheck={false}
              maxLength={32}
            />
            <Button type="button" variant="ghost" onClick={addMember}>
              <Plus className="size-4" />
              {COPY.addMember}
            </Button>
          </div>
          <ul className="mt-2 flex flex-wrap gap-1.5">
            {account ? (
              <li className="rounded-full bg-elev px-2.5 py-1 text-xs text-muted">
                {account} · {COPY.you}
              </li>
            ) : null}
            {members.map((m) => (
              <li
                key={m}
                className="inline-flex items-center gap-1 rounded-full bg-elev px-2.5 py-1 text-xs"
              >
                {m}
                <button
                  type="button"
                  className="text-muted hover:text-ink"
                  aria-label={COPY.close}
                  onClick={() => setMembers((prev) => prev.filter((x) => x !== m))}
                >
                  <X className="size-3" />
                </button>
              </li>
            ))}
          </ul>
        </Field>
        <div className="flex justify-end gap-2">
          <Button type="button" variant="ghost" onClick={() => onOpenChange(false)}>
            {COPY.cancel}
          </Button>
          <Button type="submit" disabled={pending}>
            {pending ? <Loader2 className="size-4 animate-spin" /> : null}
            {pending ? COPY.creating : COPY.createGroup}
          </Button>
        </div>
      </form>
    </Modal>
  );
}
