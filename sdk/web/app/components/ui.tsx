import Avatar from "@mui/material/Avatar";
import Dialog from "@mui/material/Dialog";
import DialogContent from "@mui/material/DialogContent";
import DialogTitle from "@mui/material/DialogTitle";
import IconButton from "@mui/material/IconButton";
import Tooltip from "@mui/material/Tooltip";
import Close from "@mui/icons-material/Close";
import type { ReactNode } from "react";

import { COPY } from "../copy.ts";
import { avatarColor, initial } from "../lib/format.ts";

export function UserAvatar({
  name,
  size = 40,
}: {
  name: string;
  size?: number;
}) {
  return (
    <Avatar
      alt=""
      sx={{
        width: size,
        height: size,
        bgcolor: avatarColor(name),
        fontSize: Math.max(12, Math.round(size * 0.38)),
        fontWeight: 700,
        color: "#fff",
      }}
    >
      {initial(name)}
    </Avatar>
  );
}

export function IconTip({ label, children }: { label: string; children: ReactNode }) {
  return (
    <Tooltip title={label} enterDelay={300}>
      <span>{children}</span>
    </Tooltip>
  );
}

export function Modal({
  open,
  onOpenChange,
  title,
  wide,
  children,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  wide?: boolean;
  children: ReactNode;
}) {
  return (
    <Dialog
      open={open}
      onClose={() => onOpenChange(false)}
      fullWidth
      maxWidth={wide ? "sm" : "xs"}
    >
      <DialogTitle sx={{ pr: 6 }}>
        {title}
        <IconButton
          aria-label={COPY.close}
          onClick={() => onOpenChange(false)}
          sx={{ position: "absolute", right: 8, top: 8 }}
        >
          <Close fontSize="small" />
        </IconButton>
      </DialogTitle>
      <DialogContent>{children}</DialogContent>
    </Dialog>
  );
}
