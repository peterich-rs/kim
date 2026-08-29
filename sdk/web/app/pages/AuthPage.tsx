import { Navigate } from "react-router-dom";

import { AuthScreen } from "../components/AuthScreen.tsx";
import { useChat } from "../state/ChatProvider.tsx";

export function AuthPage({ mode }: { mode: "login" | "register" }) {
  const { account } = useChat();
  if (account) {
    return <Navigate to="/" replace />;
  }
  return <AuthScreen mode={mode} />;
}
