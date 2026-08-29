import { Navigate } from "react-router-dom";

import { ChatScreen } from "../components/ChatScreen.tsx";
import { useChat } from "../state/ChatProvider.tsx";

export function ChatPage() {
  const { account } = useChat();
  if (!account) {
    return <Navigate to="/login" replace />;
  }
  return <ChatScreen />;
}
