import { PROFILE_KIND_BOT } from "../../src/proto.ts";
import { COPY } from "../copy.ts";
import { Badge } from "./ui/badge.tsx";

export function BotBadge({ kind }: { kind?: number }) {
  if (kind !== PROFILE_KIND_BOT) {
    return null;
  }
  return (
    <Badge variant="secondary" className="h-4 px-1.5 text-[10px] font-medium">
      {COPY.botBadge}
    </Badge>
  );
}
