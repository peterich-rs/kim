import * as Tooltip from "@radix-ui/react-tooltip";
import { BrowserRouter, Navigate, Route, Routes } from "react-router-dom";
import { Toaster } from "sonner";

import { AuthPage } from "./pages/AuthPage.tsx";
import { ChatPage } from "./pages/ChatPage.tsx";
import { ChatProvider } from "./state/ChatProvider.tsx";

export function App() {
  return (
    <Tooltip.Provider delayDuration={250}>
      <ChatProvider>
        <BrowserRouter>
          <Routes>
            <Route path="/login" element={<AuthPage mode="login" />} />
            <Route path="/register" element={<AuthPage mode="register" />} />
            <Route path="/" element={<ChatPage />} />
            <Route path="*" element={<Navigate to="/" replace />} />
          </Routes>
        </BrowserRouter>
        <Toaster
          theme="dark"
          position="top-center"
          toastOptions={{
            className: "border border-line bg-panel text-ink",
          }}
        />
      </ChatProvider>
    </Tooltip.Provider>
  );
}
