import { Check } from "lucide-react";
import { useState, type FormEvent } from "react";

import { COPY } from "../copy.ts";
import { mapUserError } from "../lib/errors.ts";
import { cn } from "../lib/utils.ts";
import { useChat } from "../state/ChatProvider.tsx";
import { Modal, UserAvatar } from "./ui.tsx";
import { Button } from "./ui/button.tsx";
import { DialogFooter } from "./ui/dialog.tsx";
import { Field, FieldError, FieldLabel } from "./ui/field.tsx";
import { Input } from "./ui/input.tsx";
import { Item, ItemContent, ItemMedia, ItemTitle } from "./ui/item.tsx";
import { ScrollArea } from "./ui/scroll-area.tsx";
import { Spinner } from "./ui/spinner.tsx";

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
        <Field>
          <FieldLabel htmlFor="group-name">{COPY.groupName}</FieldLabel>
          <Input
            id="group-name"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder={COPY.groupNamePlaceholder}
            maxLength={32}
            autoFocus
          />
        </Field>
        <div>
          <p className="mb-2 text-xs text-muted-foreground">{COPY.pickFriends}</p>
          <ScrollArea className="h-48 rounded-lg border border-border">
            {people.length === 0 ? (
              <p className="px-4 py-8 text-center text-xs text-muted-foreground">{COPY.noFriendsHint}</p>
            ) : (
              people.map((p) => {
                const on = members.includes(p.account);
                return (
                  <Item
                    key={p.account}
                    render={<button type="button" />}
                    size="sm"
                    className={cn("w-full", on && "bg-accent")}
                    onClick={() => toggleMember(p.account)}
                  >
                    <ItemMedia>
                      <UserAvatar name={p.nickname} />
                    </ItemMedia>
                    <ItemContent>
                      <ItemTitle>{p.nickname}</ItemTitle>
                    </ItemContent>
                    {on ? <Check className="size-4 text-primary" /> : null}
                  </Item>
                );
              })
            )}
          </ScrollArea>
          {account ? (
            <p className="mt-2 text-xs text-muted-foreground">
              {account} · {COPY.you}
            </p>
          ) : null}
          {error ? <FieldError className="mt-2">{error}</FieldError> : null}
        </div>
        <DialogFooter className="mx-0 mb-0 rounded-none border-0 bg-transparent p-0">
          <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
            {COPY.cancel}
          </Button>
          <Button type="submit" disabled={pending}>
            {pending ? <Spinner /> : null}
            {pending ? COPY.creating : COPY.createGroup}
          </Button>
        </DialogFooter>
      </form>
    </Modal>
  );
}
