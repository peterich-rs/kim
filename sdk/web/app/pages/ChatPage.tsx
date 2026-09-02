import { Navigate } from "react-router-dom";

import { ChatScreen } from "../components/ChatScreen.tsx";
import { KickScreen } from "../components/KickScreen.tsx";
import { useChat } from "../state/ChatProvider.tsx";

export function ChatPage() {
  const { account, kicked } = useChat();
  if (kicked) {
    return <KickScreen />;
  }
  if (!account) {
    return <Navigate to="/login" replace />;
  }
  return <ChatScreen />;
}
