import { Search, UserPlus, Users } from "lucide-react";
import { useEffect, useState, type FormEvent } from "react";
import { toast } from "sonner";

import { COPY } from "../copy.ts";
import { useChat, type Person } from "../state/ChatProvider.tsx";
import { Modal, UserAvatar } from "./ui.tsx";
import { Badge } from "./ui/badge.tsx";
import { Button } from "./ui/button.tsx";
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "./ui/empty.tsx";
import { InputGroup, InputGroupAddon, InputGroupInput } from "./ui/input-group.tsx";
import { Item, ItemActions, ItemContent, ItemDescription, ItemMedia, ItemTitle } from "./ui/item.tsx";
import { ScrollArea } from "./ui/scroll-area.tsx";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "./ui/tabs.tsx";

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
      <Tabs
        value={tab}
        onValueChange={(value) => {
          if (value === "friends" || value === "add" || value === "incoming") {
            setTab(value);
          }
        }}
        className="gap-3"
      >
        <TabsList className="w-full">
          <TabsTrigger value="friends" className="flex-1">
            {COPY.contacts}
          </TabsTrigger>
          <TabsTrigger value="add" className="flex-1">
            {COPY.addFriend}
          </TabsTrigger>
          <TabsTrigger value="incoming" className="flex-1">
            <span className="flex items-center gap-1.5">
              {COPY.incoming}
              {incomingCount > 0 ? (
                <Badge className="h-4 min-w-4 px-1 text-[10px]">{incomingCount > 99 ? "99+" : incomingCount}</Badge>
              ) : null}
            </span>
          </TabsTrigger>
        </TabsList>

        <TabsContent value="friends">
          <ScrollArea className="h-[min(52vh,420px)]">
            {people.length === 0 ? (
              <Empty className="py-10">
                <EmptyHeader>
                  <EmptyMedia variant="icon">
                    <Users />
                  </EmptyMedia>
                  <EmptyTitle>{COPY.noFriends}</EmptyTitle>
                  <EmptyDescription>{COPY.noFriendsHint}</EmptyDescription>
                </EmptyHeader>
                <EmptyContent>
                  <Button onClick={() => setTab("add")}>
                    <UserPlus />
                    {COPY.addFriend}
                  </Button>
                </EmptyContent>
              </Empty>
            ) : (
              people.map((p) => (
                <Item
                  key={p.account}
                  render={<button type="button" />}
                  size="sm"
                  className="w-full"
                  onClick={() => openChat(p)}
                >
                  <ItemMedia>
                    <UserAvatar name={p.nickname} />
                  </ItemMedia>
                  <ItemContent>
                    <ItemTitle>{p.nickname}</ItemTitle>
                    <ItemDescription>@{p.account}</ItemDescription>
                  </ItemContent>
                  <ItemActions>
                    <span className="text-xs font-medium text-primary">{COPY.chatAction}</span>
                  </ItemActions>
                </Item>
              ))
            )}
          </ScrollArea>
        </TabsContent>

        <TabsContent value="add">
          <form className="flex flex-col gap-3" onSubmit={(e) => void onSearch(e)}>
            <InputGroup>
              <InputGroupAddon>
                <Search />
              </InputGroupAddon>
              <InputGroupInput
                value={query}
                onChange={(e) => {
                  setQuery(e.target.value);
                  setSearched(false);
                }}
                placeholder={COPY.searchPeople}
                autoFocus
              />
            </InputGroup>
            <Button type="submit">{COPY.searchPeople}</Button>
            <ScrollArea className="h-56">
              {searched && hits.length === 0 ? (
                <p className="py-10 text-center text-sm text-muted-foreground">{COPY.searchEmpty}</p>
              ) : (
                hits.map((p) => {
                  const friend = isFriend(p.account);
                  const pending = outgoing.includes(p.account);
                  return (
                    <Item key={p.account} size="sm">
                      <ItemMedia>
                        <UserAvatar name={p.nickname} />
                      </ItemMedia>
                      <ItemContent>
                        <ItemTitle>{p.nickname}</ItemTitle>
                        <ItemDescription>@{p.account}</ItemDescription>
                      </ItemContent>
                      <ItemActions>
                        {friend ? (
                          <Button size="xs" variant="outline" onClick={() => openChat(p)}>
                            {COPY.chatAction}
                          </Button>
                        ) : pending ? (
                          <span className="text-xs text-muted-foreground">{COPY.requested}</span>
                        ) : (
                          <Button
                            size="xs"
                            disabled={busy === p.account}
                            onClick={() => void onRequest(p.account)}
                          >
                            {COPY.addFriend}
                          </Button>
                        )}
                      </ItemActions>
                    </Item>
                  );
                })
              )}
            </ScrollArea>
          </form>
        </TabsContent>

        <TabsContent value="incoming">
          <ScrollArea className="h-[min(52vh,420px)]">
            {incomingPeople.length === 0 ? (
              <p className="py-16 text-center text-sm text-muted-foreground">{COPY.noIncoming}</p>
            ) : (
              incomingPeople.map((p) => (
                <Item key={p.account} size="sm">
                  <ItemMedia>
                    <UserAvatar name={p.nickname} />
                  </ItemMedia>
                  <ItemContent>
                    <ItemTitle>{p.nickname}</ItemTitle>
                    <ItemDescription>@{p.account}</ItemDescription>
                  </ItemContent>
                  <ItemActions>
                    <Button
                      size="xs"
                      variant="outline"
                      onClick={() => {
                        void rejectFriend(p.account).catch((err) =>
                          toast.error(err instanceof Error ? err.message : COPY.sendFailed),
                        );
                      }}
                    >
                      {COPY.reject}
                    </Button>
                    <Button
                      size="xs"
                      onClick={() => {
                        void acceptFriend(p.account).then(() => onOpenChange(false));
                      }}
                    >
                      {COPY.accept}
                    </Button>
                  </ItemActions>
                </Item>
              ))
            )}
          </ScrollArea>
        </TabsContent>
      </Tabs>
    </Modal>
  );
}
