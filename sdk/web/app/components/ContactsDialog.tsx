import { Search, UserPlus, Users } from "lucide-react";
import { useEffect, useState, type FormEvent } from "react";
import { toast } from "sonner";

import { COPY } from "../copy.ts";
import { useChat, type Person } from "../state/ChatProvider.tsx";
import { Avatar, Button, Modal } from "./ui.tsx";

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
      <div className="flex gap-1 rounded-xl bg-stage p-1 text-xs">
        {(
          [
            ["friends", COPY.contacts, people.length],
            ["add", COPY.addFriend, 0],
            ["incoming", COPY.incoming, incomingCount],
          ] as const
        ).map(([id, label, count]) => (
          <button
            key={id}
            type="button"
            className={`relative flex-1 rounded-lg px-2 py-1.5 transition-colors ${
              tab === id ? "bg-elev font-medium text-ink" : "text-muted hover:text-ink"
            }`}
            onClick={() => setTab(id)}
          >
            {label}
            {count > 0 ? (
              <span className="ml-1 rounded-full bg-brand px-1.5 text-[10px] font-semibold text-brand-ink">
                {count > 99 ? "99+" : count}
              </span>
            ) : null}
          </button>
        ))}
      </div>

      {tab === "friends" ? (
        <ul className="msg-scroll mt-3 flex max-h-[min(52vh,420px)] flex-col gap-0.5 overflow-y-auto">
          {people.length === 0 ? (
            <li className="flex flex-col items-center px-4 py-12 text-center">
              <span className="grid size-12 place-items-center rounded-2xl bg-stage text-brand">
                <Users className="size-5" />
              </span>
              <p className="mt-3 text-sm font-medium">{COPY.noFriends}</p>
              <p className="mt-1 text-xs text-muted">{COPY.noFriendsHint}</p>
              <Button className="mt-4 h-9 px-3 text-xs" onClick={() => setTab("add")}>
                <UserPlus className="size-3.5" />
                {COPY.addFriend}
              </Button>
            </li>
          ) : (
            people.map((p) => (
              <li key={p.account}>
                <button
                  type="button"
                  onClick={() => openChat(p)}
                  className="flex w-full items-center gap-3 rounded-xl px-2 py-2 text-left transition-colors hover:bg-elev"
                >
                  <Avatar name={p.nickname} size="sm" />
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-sm font-medium">{p.nickname}</span>
                    <span className="block truncate text-xs text-muted">@{p.account}</span>
                  </span>
                  <span className="text-xs text-brand">{COPY.chatAction}</span>
                </button>
              </li>
            ))
          )}
        </ul>
      ) : null}

      {tab === "add" ? (
        <form className="mt-3 flex flex-col gap-3" onSubmit={(e) => void onSearch(e)}>
          <label className="relative block">
            <Search className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted" />
            <input
              value={query}
              onChange={(e) => {
                setQuery(e.target.value);
                setSearched(false);
              }}
              placeholder={COPY.searchPeople}
              autoFocus
              className="h-11 w-full rounded-xl border border-line bg-elev pl-9 pr-3 text-sm placeholder:text-muted/70 focus:border-brand focus:outline-none focus:ring-2 focus:ring-brand/30"
            />
          </label>
          <Button type="submit" className="h-10">
            {COPY.searchPeople}
          </Button>
          <ul className="flex max-h-56 flex-col gap-1 overflow-y-auto">
            {searched && hits.length === 0 ? (
              <li className="py-8 text-center text-sm text-muted">{COPY.searchEmpty}</li>
            ) : (
              hits.map((p) => {
                const friend = isFriend(p.account);
                const pending = outgoing.includes(p.account);
                return (
                  <li
                    key={p.account}
                    className="flex items-center gap-3 rounded-xl border border-line/70 bg-stage px-3 py-2.5"
                  >
                    <Avatar name={p.nickname} size="sm" />
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-sm font-medium">{p.nickname}</span>
                      <span className="block truncate text-xs text-muted">@{p.account}</span>
                    </span>
                    {friend ? (
                      <Button
                        type="button"
                        variant="ghost"
                        className="h-8 px-2 text-xs"
                        onClick={() => openChat(p)}
                      >
                        {COPY.chatAction}
                      </Button>
                    ) : pending ? (
                      <span className="text-xs text-muted">{COPY.requested}</span>
                    ) : (
                      <Button
                        type="button"
                        className="h-8 px-3 text-xs"
                        disabled={busy === p.account}
                        onClick={() => void onRequest(p.account)}
                      >
                        {COPY.addFriend}
                      </Button>
                    )}
                  </li>
                );
              })
            )}
          </ul>
        </form>
      ) : null}

      {tab === "incoming" ? (
        <ul className="mt-3 flex max-h-[min(52vh,420px)] flex-col gap-2 overflow-y-auto">
          {incomingPeople.length === 0 ? (
            <li className="py-12 text-center text-sm text-muted">{COPY.noIncoming}</li>
          ) : (
            incomingPeople.map((p) => (
              <li
                key={p.account}
                className="flex items-center gap-3 rounded-xl border border-line/70 bg-stage px-3 py-2.5"
              >
                <Avatar name={p.nickname} size="sm" />
                <span className="min-w-0 flex-1">
                  <span className="block truncate text-sm font-medium">{p.nickname}</span>
                  <span className="block truncate text-xs text-muted">@{p.account}</span>
                </span>
                <Button
                  type="button"
                  variant="ghost"
                  className="h-8 px-2 text-xs"
                  onClick={() => {
                    void rejectFriend(p.account).catch((err) =>
                      toast.error(err instanceof Error ? err.message : COPY.sendFailed),
                    );
                  }}
                >
                  {COPY.reject}
                </Button>
                <Button
                  type="button"
                  className="h-8 px-3 text-xs"
                  onClick={() => {
                    void acceptFriend(p.account).then(() => onOpenChange(false));
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
