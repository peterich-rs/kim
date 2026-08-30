import { useEffect, useState, type FormEvent } from "react";
import { toast } from "sonner";

import { COPY } from "../copy.ts";
import { useChat } from "../state/ChatProvider.tsx";
import { Button, Field, Modal, TextInput } from "./ui.tsx";

export function ContactsDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const { friends, incoming, searchUsers, requestFriend, acceptFriend, openThread } = useChat();
  const [tab, setTab] = useState<"friends" | "add" | "incoming">("friends");
  const [people, setPeople] = useState<{ account: string; nickname: string }[]>([]);
  const [pending, setPending] = useState<{ account: string; nickname: string }[]>([]);
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<{ account: string; nickname: string }[]>([]);

  useEffect(() => {
    if (!open) {
      return;
    }
    void friends().then(setPeople);
    void incoming().then(setPending);
  }, [open, friends, incoming]);

  async function onSearch(ev: FormEvent) {
    ev.preventDefault();
    const q = query.trim();
    if (!q) {
      return;
    }
    setHits(await searchUsers(q));
  }

  return (
    <Modal open={open} onOpenChange={onOpenChange} title={COPY.contacts}>
      <div className="flex gap-1 rounded-xl bg-stage p-1 text-xs">
        {(["friends", "add", "incoming"] as const).map((id) => (
          <button
            key={id}
            type="button"
            className={`flex-1 rounded-lg px-2 py-1.5 ${tab === id ? "bg-elev font-medium" : "text-muted"}`}
            onClick={() => setTab(id)}
          >
            {id === "friends" ? COPY.contacts : id === "add" ? COPY.addFriend : COPY.incoming}
            {id === "incoming" && pending.length > 0 ? ` (${pending.length})` : ""}
          </button>
        ))}
      </div>

      {tab === "friends" ? (
        <ul className="flex max-h-64 flex-col gap-1 overflow-y-auto">
          {people.length === 0 ? (
            <li className="py-8 text-center text-sm text-muted">{COPY.noFriends}</li>
          ) : (
            people.map((p) => (
              <li key={p.account}>
                <Button
                  variant="ghost"
                  className="w-full justify-start"
                  onClick={() => {
                    openThread(p.account, "user", p.nickname);
                    onOpenChange(false);
                  }}
                >
                  {p.nickname}
                  <span className="text-muted">@{p.account}</span>
                </Button>
              </li>
            ))
          )}
        </ul>
      ) : null}

      {tab === "add" ? (
        <form className="flex flex-col gap-3" onSubmit={(e) => void onSearch(e)}>
          <Field label={COPY.searchPeople}>
            <TextInput
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder={COPY.searchPeople}
              autoFocus
            />
          </Field>
          <Button type="submit">{COPY.addFriend}</Button>
          <ul className="flex flex-col gap-1">
            {hits.map((p) => (
              <li key={p.account} className="flex items-center justify-between gap-2 text-sm">
                <span>
                  {p.nickname} <span className="text-muted">@{p.account}</span>
                </span>
                <Button
                  type="button"
                  variant="ghost"
                  className="h-8 px-2"
                  onClick={() => {
                    void requestFriend(p.account).catch((err) =>
                      toast.error(err instanceof Error ? err.message : COPY.sendFailed),
                    );
                  }}
                >
                  {COPY.addFriend}
                </Button>
              </li>
            ))}
          </ul>
        </form>
      ) : null}

      {tab === "incoming" ? (
        <ul className="flex flex-col gap-2">
          {pending.length === 0 ? (
            <li className="py-8 text-center text-sm text-muted">{COPY.noFriendsHint}</li>
          ) : (
            pending.map((p) => (
              <li key={p.account} className="flex items-center justify-between gap-2 text-sm">
                <span>
                  {p.nickname} <span className="text-muted">@{p.account}</span>
                </span>
                <Button
                  type="button"
                  className="h-8 px-3"
                  onClick={() => {
                    void acceptFriend(p.account).then(() => {
                      setPending((cur) => cur.filter((x) => x.account !== p.account));
                      onOpenChange(false);
                    });
                  }}
                >
                  {COPY.accept}
                </Button>
              </li>
            ))
          )}
        </ul>
      ) : null}
    </Modal>
  );
}
