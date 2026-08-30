import { Loader2, X } from "lucide-react";
import { useState, type FormEvent } from "react";

import { COPY } from "../copy.ts";
import { mapUserError } from "../lib/errors.ts";
import { useChat } from "../state/ChatProvider.tsx";
import { Avatar, Button, Field, Modal, TextInput } from "./ui.tsx";

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
        <Field label={COPY.groupMembers} error={error} hint={COPY.pickFriends}>
          <ul className="msg-scroll flex max-h-48 flex-col gap-0.5 overflow-y-auto rounded-xl border border-line bg-stage p-1">
            {people.length === 0 ? (
              <li className="px-3 py-8 text-center text-xs text-muted">{COPY.noFriendsHint}</li>
            ) : (
              people.map((p) => {
                const on = members.includes(p.account);
                return (
                  <li key={p.account}>
                    <button
                      type="button"
                      onClick={() => toggleMember(p.account)}
                      className={`flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-left text-sm ${
                        on ? "bg-elev" : "hover:bg-elev/70"
                      }`}
                    >
                      <Avatar name={p.nickname} size="sm" />
                      <span className="min-w-0 flex-1 truncate">{p.nickname}</span>
                      {on ? <X className="size-3 text-muted" /> : null}
                    </button>
                  </li>
                );
              })
            )}
          </ul>
          {account ? (
            <p className="mt-1 text-xs text-muted">
              {account} · {COPY.you}
            </p>
          ) : null}
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
