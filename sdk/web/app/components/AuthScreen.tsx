import { Eye, EyeOff, Loader2 } from "lucide-react";
import { useState, type FormEvent } from "react";
import { Link, useLocation, useNavigate } from "react-router-dom";

import { COPY } from "../copy.ts";
import { mapUserError } from "../lib/errors.ts";
import { validateAccount, validateConfirm, validatePassword } from "../lib/validation.ts";
import { useChat } from "../state/ChatProvider.tsx";
import { Logo } from "./Logo.tsx";
import { Button, Field, TextInput } from "./ui.tsx";

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
    <div className="auth-bg grid min-h-dvh lg:grid-cols-[1fr_440px]">
      <aside className="hidden flex-col p-12 lg:flex">
        <Logo />
        <div className="mt-auto pb-8">
          <h1 className="text-5xl font-semibold tracking-tight">{COPY.brand}</h1>
          <p className="mt-3 text-lg text-muted">{COPY.brandSub}</p>
        </div>
      </aside>

      <section className="flex items-center justify-center px-5 py-10">
        <div className="w-full max-w-[400px] rounded-2xl border border-line bg-panel p-7 shadow-[0_24px_80px_rgb(0_0_0/0.35)]">
          <div className="mb-6 lg:hidden">
            <Logo />
          </div>
          <h2 className="text-xl font-semibold">{isRegister ? COPY.registerTitle : COPY.loginTitle}</h2>
          <form className="mt-6 flex flex-col gap-4" onSubmit={(ev) => void onSubmit(ev)} noValidate>
            <Field label={COPY.account} hint={COPY.accountHint} error={fieldErr.account}>
              <TextInput
                name="username"
                autoComplete="username"
                spellCheck={false}
                maxLength={32}
                value={account}
                onChange={(e) => setAccount(e.target.value)}
                placeholder={COPY.accountPlaceholder}
                autoFocus
              />
            </Field>
            <Field label={COPY.password} hint={isRegister ? COPY.passwordHint : undefined} error={fieldErr.password}>
              <div className="relative">
                <TextInput
                  name="password"
                  type={showPw ? "text" : "password"}
                  autoComplete={isRegister ? "new-password" : "current-password"}
                  maxLength={128}
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  placeholder={COPY.passwordPlaceholder}
                  className="pr-11"
                />
                <button
                  type="button"
                  className="absolute right-2 top-1/2 -translate-y-1/2 rounded-lg p-1.5 text-muted hover:text-ink"
                  aria-label={showPw ? COPY.hidePassword : COPY.showPassword}
                  onClick={() => setShowPw((v) => !v)}
                >
                  {showPw ? <EyeOff className="size-4" /> : <Eye className="size-4" />}
                </button>
              </div>
            </Field>
            {isRegister ? (
              <Field label={COPY.confirmPassword} error={fieldErr.confirm}>
                <TextInput
                  type={showPw ? "text" : "password"}
                  autoComplete="new-password"
                  maxLength={128}
                  value={confirm}
                  onChange={(e) => setConfirm(e.target.value)}
                  placeholder={COPY.confirmPlaceholder}
                />
              </Field>
            ) : null}
            {error ? (
              <p className="rounded-lg bg-danger/10 px-3 py-2 text-sm text-danger" role="alert">
                {error}
              </p>
            ) : null}
            <Button type="submit" disabled={pending} className="mt-1 h-11 w-full">
              {pending ? <Loader2 className="size-4 animate-spin" /> : null}
              {pending
                ? isRegister
                  ? COPY.submittingRegister
                  : COPY.submittingLogin
                : isRegister
                  ? COPY.registerAction
                  : COPY.loginAction}
            </Button>
          </form>
          <p className="mt-5 text-center text-sm text-muted">
            {isRegister ? COPY.hasAccount : COPY.noAccount}{" "}
            <Link
              to={isRegister ? "/login" : "/register"}
              className="font-medium text-brand hover:underline"
            >
              {isRegister ? COPY.goLogin : COPY.goRegister}
            </Link>
          </p>
        </div>
      </section>
    </div>
  );
}
