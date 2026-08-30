import * as Dialog from "@radix-ui/react-dialog";
import { X } from "lucide-react";

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
    <Dialog.Root
      open={open && Boolean(src)}
      onOpenChange={(next) => {
        if (!next) {
          onClose();
        }
      }}
    >
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-50 bg-black/80" />
        <Dialog.Content
          className="fixed inset-0 z-50 grid place-items-center p-4 outline-none"
          onClick={onClose}
          aria-label={COPY.viewImage}
        >
          <Dialog.Title className="sr-only">{COPY.viewImage}</Dialog.Title>
          {src ? (
            <img
              src={src}
              alt=""
              className="max-h-[92dvh] max-w-[92vw] object-contain shadow-2xl"
              onClick={(ev) => ev.stopPropagation()}
            />
          ) : null}
          <Dialog.Close
            className="absolute right-4 top-4 grid size-10 place-items-center rounded-full bg-black/50 text-white hover:bg-black/70"
            aria-label={COPY.closeViewer}
            onClick={onClose}
          >
            <X className="size-5" />
          </Dialog.Close>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
