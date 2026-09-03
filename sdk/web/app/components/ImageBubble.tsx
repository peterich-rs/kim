import { COPY } from "../copy.ts";
import { bubbleSize, type ImageSize } from "../lib/image.ts";

export function ImageBubble({
  src,
  size,
  mine,
  last = true,
  onOpen,
}: {
  src: string;
  size?: ImageSize;
  mine: boolean;
  last?: boolean;
  onOpen: (src: string) => void;
}) {
  const dim = bubbleSize(size);
  void mine;
  void last;
  return (
    <button
      type="button"
      onClick={() => onOpen(src)}
      aria-label={COPY.viewImage}
      className="block cursor-pointer overflow-hidden bg-background p-0"
      style={{ width: dim.width, height: dim.height }}
    >
      <img
        src={src}
        alt=""
        width={dim.width}
        height={dim.height}
        loading="lazy"
        decoding="async"
        className="block size-full object-cover"
      />
      {last && mine ? null : null}
    </button>
  );
}
