import Box from "@mui/material/Box";

import { COPY } from "../copy.ts";
import { bubbleSize, type ImageSize } from "../lib/image.ts";

export function ImageBubble({
  src,
  size,
  mine,
  onOpen,
}: {
  src: string;
  size?: ImageSize;
  mine: boolean;
  onOpen: (src: string) => void;
}) {
  const dim = bubbleSize(size);
  return (
    <Box
      component="button"
      type="button"
      onClick={() => onOpen(src)}
      aria-label={COPY.viewImage}
      sx={{
        display: "block",
        p: 0,
        border: 0,
        overflow: "hidden",
        borderRadius: 2,
        borderTopRightRadius: mine ? "4px" : 16,
        borderTopLeftRadius: mine ? 16 : "4px",
        bgcolor: "background.paper",
        width: dim.width,
        height: dim.height,
        cursor: "pointer",
      }}
    >
      <Box
        component="img"
        src={src}
        alt=""
        width={dim.width}
        height={dim.height}
        loading="lazy"
        decoding="async"
        sx={{ width: "100%", height: "100%", objectFit: "cover", display: "block" }}
      />
    </Box>
  );
}
