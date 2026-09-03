import { useNavigate } from "react-router-dom";

import { COPY } from "../copy.ts";
import { useChat } from "../state/ChatProvider.tsx";
import { Logo } from "./Logo.tsx";
import { Button } from "./ui/button.tsx";
import { Card, CardContent } from "./ui/card.tsx";

export function KickScreen() {
  const { dismissKick } = useChat();
  const navigate = useNavigate();

  return (
    <div className="relative grid min-h-dvh place-items-center bg-background px-4">
      <div
        aria-hidden
        className="pointer-events-none absolute inset-0"
        style={{
          backgroundImage:
            "radial-gradient(900px 520px at 12% -8%, color-mix(in oklch, var(--primary) 18%, transparent), transparent 58%), radial-gradient(720px 420px at 100% 108%, color-mix(in oklch, var(--primary) 10%, transparent), transparent 52%)",
        }}
      />
      <Card className="relative w-full max-w-[25rem] text-center shadow-sm">
        <CardContent className="flex flex-col items-center gap-3 pt-8 pb-6">
          <Logo />
          <h1 className="text-lg font-semibold">{COPY.kickedTitle}</h1>
          <p className="text-sm text-muted-foreground">{COPY.kickedHint}</p>
          <Button
            className="mt-2 w-full"
            onClick={() => {
              dismissKick();
              navigate("/login", { replace: true });
            }}
          >
            {COPY.kickedAction}
          </Button>
        </CardContent>
      </Card>
    </div>
  );
}
