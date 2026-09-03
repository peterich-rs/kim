import { X } from "lucide-react";

import { COPY } from "../copy.ts";
import { Button } from "./ui/button.tsx";
import { Dialog, DialogContent } from "./ui/dialog.tsx";

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
    <Dialog open={open && Boolean(src)} onOpenChange={(next) => !next && onClose()}>
      <DialogContent
        showCloseButton={false}
        className="max-w-[92vw] border-0 bg-transparent p-0 shadow-none ring-0 sm:max-w-[92vw]"
      >
        <div className="relative" onClick={onClose}>
          {src ? (
            <img
              src={src}
              alt=""
              onClick={(ev) => ev.stopPropagation()}
              className="block max-h-[92dvh] max-w-[92vw] object-contain"
            />
          ) : null}
          <Button
            type="button"
            size="icon-sm"
            variant="ghost"
            aria-label={COPY.closeViewer}
            onClick={onClose}
            className="absolute top-2 right-2 bg-black/50 text-white hover:bg-black/70 hover:text-white"
          >
            <X />
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
