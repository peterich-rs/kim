import Alert from "@mui/material/Alert";
import Box from "@mui/material/Box";
import Button from "@mui/material/Button";
import { useState } from "react";
import { useMediaQuery } from "usehooks-ts";

import { COPY } from "../copy.ts";
import { useChat } from "../state/ChatProvider.tsx";
import { ContactsDialog } from "./ContactsDialog.tsx";
import { ConversationList } from "./ConversationList.tsx";
import { MessagePane } from "./MessagePane.tsx";
import { NewGroupDialog } from "./NewGroupDialog.tsx";
import { ProfileDialog } from "./ProfileDialog.tsx";

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
    <Box sx={{ display: "flex", flexDirection: "column", height: "100dvh", minHeight: 0, bgcolor: (theme) => theme.palette.canvas }}>
      {banner && status !== "online" ? (
        <Alert
          severity={status === "offline" ? "warning" : "info"}
          action={
            status === "offline" ? (
              <Button color="inherit" size="small" onClick={() => void connect()}>
                {COPY.retry}
              </Button>
            ) : null
          }
          sx={{ borderRadius: 0, py: 0 }}
        >
          {banner}
        </Alert>
      ) : null}
      <Box sx={{ display: "flex", flex: 1, minHeight: 0 }}>
        <Box sx={{ display: showList ? "flex" : "none", height: "100%", minHeight: 0 }}>
          <ConversationList
            onNewChat={() => openContacts("friends")}
            onAddFriend={() => openContacts("add")}
            onNewGroup={() => setNewGroup(true)}
            onProfile={() => setProfile(true)}
          />
        </Box>
        <Box sx={{ display: showPane ? "flex" : "none", flex: 1, minWidth: 0, minHeight: 0 }}>
          <MessagePane />
        </Box>
      </Box>
      <ContactsDialog open={contacts} onOpenChange={setContacts} initialTab={contactsTab} />
      <NewGroupDialog open={newGroup} onOpenChange={setNewGroup} />
      <ProfileDialog open={profile} onOpenChange={setProfile} />
    </Box>
  );
}
