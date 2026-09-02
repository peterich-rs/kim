import Close from "@mui/icons-material/Close";
import Dialog from "@mui/material/Dialog";
import IconButton from "@mui/material/IconButton";
import Box from "@mui/material/Box";

import { COPY } from "../copy.ts";

export function ImageViewer({
  src,
  open,
  onClose,
}: {
  src: string | null;
  open: boolean;
  onClose: () => void;
}) {
  return (
    <Dialog
      open={open && Boolean(src)}
      onClose={onClose}
      maxWidth={false}
      slotProps={{
        paper: {
          sx: { bgcolor: "transparent", boxShadow: "none", overflow: "visible" },
        },
      }}
    >
      <Box onClick={onClose} sx={{ position: "relative", outline: "none" }} aria-label={COPY.viewImage}>
        {src ? (
          <Box
            component="img"
            src={src}
            alt=""
            onClick={(ev) => ev.stopPropagation()}
            sx={{ maxHeight: "92dvh", maxWidth: "92vw", objectFit: "contain", display: "block" }}
          />
        ) : null}
        <IconButton
          aria-label={COPY.closeViewer}
          onClick={onClose}
          sx={{ position: "absolute", top: 8, right: 8, bgcolor: "rgba(0,0,0,0.5)", color: "#fff", "&:hover": { bgcolor: "rgba(0,0,0,0.7)" } }}
        >
          <Close />
        </IconButton>
      </Box>
    </Dialog>
  );
}
