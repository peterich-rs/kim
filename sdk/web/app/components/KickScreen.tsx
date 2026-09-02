import Box from "@mui/material/Box";
import Button from "@mui/material/Button";
import Paper from "@mui/material/Paper";
import Typography from "@mui/material/Typography";
import { useNavigate } from "react-router-dom";

import { COPY } from "../copy.ts";
import { useChat } from "../state/ChatProvider.tsx";
import { Logo } from "./Logo.tsx";

export function KickScreen() {
  const { dismissKick } = useChat();
  const navigate = useNavigate();

  return (
    <Box
      sx={{
        minHeight: "100dvh",
        display: "grid",
        placeItems: "center",
        bgcolor: (theme) => theme.palette.canvas,
        px: 2,
      }}
    >
      <Paper sx={{ p: 4, maxWidth: 400, width: "100%", textAlign: "center" }}>
        <Box sx={{ display: "flex", justifyContent: "center", mb: 2 }}>
          <Logo />
        </Box>
        <Typography variant="h6" sx={{ mb: 1 }}>
          {COPY.kickedTitle}
        </Typography>
        <Typography variant="body2" color="text.secondary" sx={{ mb: 3 }}>
          {COPY.kickedHint}
        </Typography>
        <Button
          variant="contained"
          fullWidth
          onClick={() => {
            dismissKick();
            navigate("/login", { replace: true });
          }}
        >
          {COPY.kickedAction}
        </Button>
      </Paper>
    </Box>
  );
}
