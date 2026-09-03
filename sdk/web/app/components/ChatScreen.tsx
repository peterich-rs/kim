import { useState } from "react";
import { useMediaQuery } from "usehooks-ts";

import { COPY } from "../copy.ts";
import { useChat } from "../state/ChatProvider.tsx";
import { ContactsDialog } from "./ContactsDialog.tsx";
import { ConversationList } from "./ConversationList.tsx";
import { MessagePane } from "./MessagePane.tsx";
import { NewGroupDialog } from "./NewGroupDialog.tsx";
import { ProfileDialog } from "./ProfileDialog.tsx";
import { Alert, AlertAction, AlertDescription } from "./ui/alert.tsx";
import { Button } from "./ui/button.tsx";

export function ChatScreen() {
  const { activeId, status, connectError, connect } = useChat();
  const [contacts, setContacts] = useState(false);
  const [contactsTab, setContactsTab] = useState<"friends" | "add" | "incoming">("friends");
  const [newGroup, setNewGroup] = useState(false);
  const [profile, setProfile] = useState(false);
  const isMobile = useMediaQuery("(max-width: 899px)");

  function openContacts(tab: "friends" | "add" | "incoming") {
    setContactsTab(tab);
    setContacts(true);
  }

  const banner =
    connectError ??
    (status === "connecting"
      ? COPY.connecting
      : status === "reconnecting"
        ? COPY.reconnectHint
        : status === "offline"
          ? COPY.offlineHint
          : null);

  const showList = !isMobile || !activeId;
  const showPane = !isMobile || Boolean(activeId);

  return (
    <div className="flex h-dvh min-h-0 flex-col bg-background">
      {banner && status !== "online" ? (
        <Alert className="rounded-none border-x-0 border-t-0 py-2" variant={status === "offline" ? "destructive" : "default"}>
          <AlertDescription>{banner}</AlertDescription>
          {status === "offline" ? (
            <AlertAction>
              <Button variant="outline" size="xs" onClick={() => void connect()}>
                {COPY.retry}
              </Button>
            </AlertAction>
          ) : null}
        </Alert>
      ) : null}
      <div className="flex min-h-0 flex-1">
        <div className={showList ? "flex h-full min-h-0" : "hidden"}>
          <ConversationList
            onNewChat={() => openContacts("friends")}
            onAddFriend={() => openContacts("add")}
            onNewGroup={() => setNewGroup(true)}
            onProfile={() => setProfile(true)}
          />
        </div>
        <div className={showPane ? "flex min-h-0 min-w-0 flex-1" : "hidden"}>
          <MessagePane />
        </div>
      </div>
      <ContactsDialog open={contacts} onOpenChange={setContacts} initialTab={contactsTab} />
      <NewGroupDialog open={newGroup} onOpenChange={setNewGroup} />
      <ProfileDialog open={profile} onOpenChange={setProfile} />
    </div>
  );
}
