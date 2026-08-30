import { useState } from "react";

import { COPY } from "../copy.ts";
import { cn } from "../lib/cn.ts";
import { useChat } from "../state/ChatProvider.tsx";
import { ContactsDialog } from "./ContactsDialog.tsx";
import { ConversationList } from "./ConversationList.tsx";
import { MessagePane } from "./MessagePane.tsx";
import { NewGroupDialog } from "./NewGroupDialog.tsx";
import { ProfileDialog } from "./ProfileDialog.tsx";
import { Button } from "./ui.tsx";

export function ChatScreen() {
  const { activeId, status, connectError, connect } = useChat();
  const [contacts, setContacts] = useState(false);
  const [contactsTab, setContactsTab] = useState<"friends" | "add" | "incoming">("friends");
  const [newGroup, setNewGroup] = useState(false);
  const [profile, setProfile] = useState(false);

  function openContacts(tab: "friends" | "add" | "incoming") {
    setContactsTab(tab);
    setContacts(true);
  }

  const banner =
    connectError ??
    (status === "connecting"
      ? COPY.connecting
      : status === "reconnecting"
        ? COPY.reconnecting
        : status === "offline"
          ? COPY.offline
          : null);

  return (
    <div className="relative flex h-dvh min-h-0 bg-stage">
      <div className={cn("flex h-full min-h-0 w-full md:w-auto", activeId && "hidden md:flex")}>
        <ConversationList
          onNewChat={() => openContacts("friends")}
          onAddFriend={() => openContacts("add")}
          onNewGroup={() => setNewGroup(true)}
          onProfile={() => setProfile(true)}
        />
      </div>
      <div className={cn("relative min-h-0 min-w-0 flex-1", !activeId && "hidden md:flex")}>
        <div className="flex h-full min-h-0 w-full flex-col">
          {banner && status !== "online" ? (
            <div className="flex items-center justify-center gap-3 border-b border-line bg-elev px-3 py-1.5 text-xs text-muted">
              <span>{banner}</span>
              {status === "offline" ? (
                <Button variant="ghost" className="h-7 px-2 py-0 text-xs" onClick={() => void connect()}>
                  {COPY.retry}
                </Button>
              ) : null}
            </div>
          ) : null}
          <MessagePane />
        </div>
      </div>
      <ContactsDialog
        open={contacts}
        onOpenChange={setContacts}
        initialTab={contactsTab}
      />
      <NewGroupDialog open={newGroup} onOpenChange={setNewGroup} />
      <ProfileDialog open={profile} onOpenChange={setProfile} />
    </div>
  );
}
