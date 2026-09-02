import CssBaseline from "@mui/material/CssBaseline";
import { ThemeProvider, useColorScheme } from "@mui/material/styles";
import { BrowserRouter, Navigate, Route, Routes } from "react-router-dom";
import { Toaster } from "sonner";

import { AuthPage } from "./pages/AuthPage.tsx";
import { ChatPage } from "./pages/ChatPage.tsx";
import { ChatProvider } from "./state/ChatProvider.tsx";
import { theme } from "./theme.ts";

function ThemedToaster() {
  const { mode, systemMode } = useColorScheme();
  const resolved = mode === "system" ? systemMode : mode;
  return (
    <Toaster
      theme={resolved === "dark" ? "dark" : "light"}
      position="top-center"
      richColors
      closeButton
    />
  );
}

export function App() {
  return (
    <ThemeProvider theme={theme} defaultMode="system">
      <CssBaseline enableColorScheme />
      <ChatProvider>
        <BrowserRouter>
          <Routes>
            <Route path="/login" element={<AuthPage mode="login" />} />
            <Route path="/register" element={<AuthPage mode="register" />} />
            <Route path="/" element={<ChatPage />} />
            <Route path="*" element={<Navigate to="/" replace />} />
          </Routes>
        </BrowserRouter>
        <ThemedToaster />
      </ChatProvider>
    </ThemeProvider>
  );
}
