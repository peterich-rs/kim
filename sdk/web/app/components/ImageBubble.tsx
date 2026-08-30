import { bubbleSize, type ImageSize } from "../lib/image.ts";
import { COPY } from "../copy.ts";

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
    <button
      type="button"
      className={`overflow-hidden rounded-2xl bg-elev ${mine ? "rounded-tr-md" : "rounded-tl-md"}`}
      style={{ width: dim.width, height: dim.height }}
      onClick={() => onOpen(src)}
      aria-label={COPY.viewImage}
    >
      <img
        src={src}
        alt=""
        width={dim.width}
        height={dim.height}
        loading="lazy"
        decoding="async"
        className="h-full w-full object-cover"
      />
    </button>
  );
}
