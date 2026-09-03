import { ThemeProvider } from "next-themes";
import { BrowserRouter, Navigate, Route, Routes } from "react-router-dom";

import { Toaster } from "./components/ui/sonner.tsx";
import { TooltipProvider } from "./components/ui/tooltip.tsx";
import { AuthPage } from "./pages/AuthPage.tsx";
import { ChatPage } from "./pages/ChatPage.tsx";
import { ChatProvider } from "./state/ChatProvider.tsx";

export function App() {
  return (
    <ThemeProvider attribute="class" defaultTheme="system" enableSystem storageKey="kim-theme" disableTransitionOnChange>
      <TooltipProvider delay={300}>
        <ChatProvider>
          <BrowserRouter>
            <Routes>
              <Route path="/login" element={<AuthPage mode="login" />} />
              <Route path="/register" element={<AuthPage mode="register" />} />
              <Route path="/" element={<ChatPage />} />
              <Route path="*" element={<Navigate to="/" replace />} />
            </Routes>
          </BrowserRouter>
          <Toaster position="top-center" richColors closeButton />
        </ChatProvider>
      </TooltipProvider>
    </ThemeProvider>
  );
}
