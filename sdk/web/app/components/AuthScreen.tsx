import { Eye, EyeOff } from "lucide-react";
import { useState, type FormEvent } from "react";
import { Link as RouterLink, useLocation, useNavigate } from "react-router-dom";

import { COPY } from "../copy.ts";
import { mapUserError } from "../lib/errors.ts";
import { validateAccount, validateConfirm, validatePassword } from "../lib/validation.ts";
import { useChat } from "../state/ChatProvider.tsx";
import { Logo } from "./Logo.tsx";
import { Alert, AlertDescription } from "./ui/alert.tsx";
import { Button } from "./ui/button.tsx";
import { Card, CardContent } from "./ui/card.tsx";
import { Field, FieldDescription, FieldError, FieldGroup, FieldLabel } from "./ui/field.tsx";
import { Input } from "./ui/input.tsx";
import { InputGroup, InputGroupAddon, InputGroupButton, InputGroupInput } from "./ui/input-group.tsx";
import { Spinner } from "./ui/spinner.tsx";

export function AuthScreen({ mode }: { mode: "login" | "register" }) {
  const { signIn } = useChat();
  const navigate = useNavigate();
  const location = useLocation();
  const notice =
    typeof location.state === "object" &&
    location.state !== null &&
    "notice" in location.state &&
    typeof location.state.notice === "string"
      ? location.state.notice
      : "";

  const [account, setAccount] = useState("");
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [showPw, setShowPw] = useState(false);
  const [error, setError] = useState(notice);
  const [pending, setPending] = useState(false);
  const [fieldErr, setFieldErr] = useState<{ account?: string; password?: string; confirm?: string }>(
    {},
  );

  const isRegister = mode === "register";

  async function onSubmit(ev: FormEvent) {
    ev.preventDefault();
    const nextErr = {
      account: validateAccount(account),
      password: validatePassword(password),
      confirm: isRegister ? validateConfirm(password, confirm) : undefined,
    };
    setFieldErr(nextErr);
    if (nextErr.account || nextErr.password || nextErr.confirm) {
      setError("");
      return;
    }
    setPending(true);
    setError("");
    try {
      await signIn(mode, account.trim(), password);
      navigate("/", { replace: true });
    } catch (err) {
      setError(mapUserError(err));
    } finally {
      setPending(false);
    }
  }

  return (
    <div className="relative grid min-h-dvh place-items-center bg-background px-4 py-16">
      <div
        aria-hidden
        className="pointer-events-none absolute inset-0"
        style={{
          backgroundImage:
            "radial-gradient(900px 520px at 12% -8%, color-mix(in oklch, var(--primary) 18%, transparent), transparent 58%), radial-gradient(720px 420px at 100% 108%, color-mix(in oklch, var(--primary) 10%, transparent), transparent 52%)",
        }}
      />
      <Card className="relative w-full max-w-[26rem] border-border/80 shadow-sm">
        <CardContent className="pt-6">
          <div className="mb-6 space-y-1">
            <Logo />
            <p className="text-sm text-muted-foreground">{COPY.brandSub}</p>
          </div>
          <h1 className="text-xl font-semibold tracking-tight">
            {isRegister ? COPY.registerTitle : COPY.loginTitle}
          </h1>
          <form className="mt-6" onSubmit={(ev) => void onSubmit(ev)} noValidate>
            <FieldGroup>
              <Field data-invalid={Boolean(fieldErr.account) || undefined}>
                <FieldLabel htmlFor="account">{COPY.account}</FieldLabel>
                <Input
                  id="account"
                  name="username"
                  autoComplete="username"
                  spellCheck={false}
                  maxLength={32}
                  value={account}
                  onChange={(e) => setAccount(e.target.value)}
                  placeholder={COPY.accountPlaceholder}
                  autoFocus
                  aria-invalid={Boolean(fieldErr.account) || undefined}
                />
                {fieldErr.account ? (
                  <FieldError>{fieldErr.account}</FieldError>
                ) : isRegister ? (
                  <FieldDescription>{COPY.accountHint}</FieldDescription>
                ) : null}
              </Field>
              <Field data-invalid={Boolean(fieldErr.password) || undefined}>
                <FieldLabel htmlFor="password">{COPY.password}</FieldLabel>
                <InputGroup>
                  <InputGroupInput
                    id="password"
                    name="password"
                    type={showPw ? "text" : "password"}
                    autoComplete={isRegister ? "new-password" : "current-password"}
                    value={password}
                    onChange={(e) => setPassword(e.target.value)}
                    placeholder={COPY.passwordPlaceholder}
                    maxLength={128}
                    aria-invalid={Boolean(fieldErr.password) || undefined}
                  />
                  <InputGroupAddon align="inline-end">
                    <InputGroupButton
                      size="icon-xs"
                      aria-label={showPw ? COPY.hidePassword : COPY.showPassword}
                      onClick={() => setShowPw((v) => !v)}
                    >
                      {showPw ? <EyeOff /> : <Eye />}
                    </InputGroupButton>
                  </InputGroupAddon>
                </InputGroup>
                {fieldErr.password ? (
                  <FieldError>{fieldErr.password}</FieldError>
                ) : isRegister ? (
                  <FieldDescription>{COPY.passwordHint}</FieldDescription>
                ) : null}
              </Field>
              {isRegister ? (
                <Field data-invalid={Boolean(fieldErr.confirm) || undefined}>
                  <FieldLabel htmlFor="confirm">{COPY.confirmPassword}</FieldLabel>
                  <Input
                    id="confirm"
                    type={showPw ? "text" : "password"}
                    autoComplete="new-password"
                    maxLength={128}
                    value={confirm}
                    onChange={(e) => setConfirm(e.target.value)}
                    placeholder={COPY.confirmPlaceholder}
                    aria-invalid={Boolean(fieldErr.confirm) || undefined}
                  />
                  {fieldErr.confirm ? <FieldError>{fieldErr.confirm}</FieldError> : null}
                </Field>
              ) : null}
              {error ? (
                <Alert variant="destructive">
                  <AlertDescription>{error}</AlertDescription>
                </Alert>
              ) : null}
              <Button type="submit" className="w-full" size="lg" disabled={pending}>
                {pending ? <Spinner /> : null}
                {pending
                  ? isRegister
                    ? COPY.submittingRegister
                    : COPY.submittingLogin
                  : isRegister
                    ? COPY.registerAction
                    : COPY.loginAction}
              </Button>
            </FieldGroup>
          </form>
          <p className="mt-6 text-center text-sm text-muted-foreground">
            {isRegister ? COPY.hasAccount : COPY.noAccount}{" "}
            <RouterLink
              to={isRegister ? "/login" : "/register"}
              className="font-medium text-primary hover:underline"
            >
              {isRegister ? COPY.goLogin : COPY.goRegister}
            </RouterLink>
          </p>
        </CardContent>
      </Card>
    </div>
  );
}
