import InitColorSchemeScript from "@mui/material/InitColorSchemeScript";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "./App.tsx";
import "./styles.css";

const root = document.getElementById("root");
if (!root) {
  throw new Error("#root missing");
}

createRoot(root).render(
  <StrictMode>
    <InitColorSchemeScript defaultMode="system" attribute="data" />
    <App />
  </StrictMode>,
);
