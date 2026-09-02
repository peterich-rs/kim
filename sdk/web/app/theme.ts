import { createTheme } from "@mui/material/styles";

declare module "@mui/material/styles" {
  interface Palette {
    canvas: string;
    bubbleMe: string;
    bubbleThem: string;
    unread: string;
    hairline: string;
  }
  interface PaletteOptions {
    canvas?: string;
    bubbleMe?: string;
    bubbleThem?: string;
    unread?: string;
    hairline?: string;
  }
}

const fontFamily = [
  "ui-sans-serif",
  "system-ui",
  "PingFang SC",
  "Hiragino Sans GB",
  "Noto Sans SC",
  "Segoe UI",
  "sans-serif",
].join(",");

export const theme = createTheme({
  cssVariables: {
    colorSchemeSelector: "data",
  },
  colorSchemes: {
    light: {
      palette: {
        primary: { main: "#0f766e", contrastText: "#ffffff" },
        secondary: { main: "#0369a1" },
        error: { main: "#dc2626" },
        success: { main: "#16a34a" },
        warning: { main: "#d97706" },
        info: { main: "#0284c7" },
        background: { default: "#d7e3ee", paper: "#ffffff" },
        text: { primary: "#0f172a", secondary: "#5b6b7c" },
        divider: "rgba(15, 23, 42, 0.10)",
        canvas: "#d7e3ee",
        bubbleMe: "#d4f0e4",
        bubbleThem: "#ffffff",
        unread: "#0284c7",
        hairline: "rgba(15, 23, 42, 0.10)",
      },
    },
    dark: {
      palette: {
        primary: { main: "#2dd4bf", contrastText: "#042f2e" },
        secondary: { main: "#38bdf8" },
        error: { main: "#f87171" },
        success: { main: "#4ade80" },
        warning: { main: "#fbbf24" },
        info: { main: "#38bdf8" },
        background: { default: "#0e1621", paper: "#17212b" },
        text: { primary: "#e8eef4", secondary: "#8b9bb0" },
        divider: "rgba(232, 238, 244, 0.10)",
        canvas: "#0e1621",
        bubbleMe: "#1a4a4a",
        bubbleThem: "#1c2733",
        unread: "#38bdf8",
        hairline: "rgba(232, 238, 244, 0.10)",
      },
    },
  },
  typography: {
    fontFamily,
    fontSize: 14,
    button: { textTransform: "none", fontWeight: 600 },
    subtitle1: { fontWeight: 600 },
    subtitle2: { fontWeight: 600 },
  },
  shape: { borderRadius: 12 },
  components: {
    MuiCssBaseline: {
      styleOverrides: {
        html: { height: "100%" },
        body: { height: "100%", margin: 0 },
        "#root": { minHeight: "100dvh", height: "100%" },
        "*, *::before, *::after": { boxSizing: "border-box" },
      },
    },
    MuiButton: {
      defaultProps: { disableElevation: true },
      styleOverrides: {
        root: { borderRadius: 10, minHeight: 36 },
      },
    },
    MuiIconButton: {
      styleOverrides: {
        root: { borderRadius: 10 },
      },
    },
    MuiListItemButton: {
      styleOverrides: {
        root: { borderRadius: 12 },
      },
    },
    MuiDialog: {
      styleOverrides: {
        paper: { borderRadius: 16 },
      },
    },
    MuiAlert: {
      styleOverrides: {
        root: { borderRadius: 0 },
      },
    },
    MuiTooltip: {
      defaultProps: { arrow: true, enterDelay: 400 },
    },
    MuiTextField: {
      defaultProps: { size: "small", variant: "outlined" },
    },
  },
});
