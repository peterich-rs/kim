import Visibility from "@mui/icons-material/Visibility";
import VisibilityOff from "@mui/icons-material/VisibilityOff";
import Alert from "@mui/material/Alert";
import Box from "@mui/material/Box";
import Button from "@mui/material/Button";
import CircularProgress from "@mui/material/CircularProgress";
import IconButton from "@mui/material/IconButton";
import InputAdornment from "@mui/material/InputAdornment";
import Paper from "@mui/material/Paper";
import Stack from "@mui/material/Stack";
import TextField from "@mui/material/TextField";
import Typography from "@mui/material/Typography";
import { useState, type FormEvent } from "react";
import { Link as RouterLink, useLocation, useNavigate } from "react-router-dom";

import { COPY } from "../copy.ts";
import { mapUserError } from "../lib/errors.ts";
import { validateAccount, validateConfirm, validatePassword } from "../lib/validation.ts";
import { useChat } from "../state/ChatProvider.tsx";
import { Logo } from "./Logo.tsx";

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
    <Box
      sx={{
        minHeight: "100dvh",
        display: "grid",
        gridTemplateColumns: { xs: "1fr", lg: "1fr 440px" },
        bgcolor: (theme) => theme.palette.canvas,
      }}
    >
      <Box sx={{ display: { xs: "none", lg: "flex" }, flexDirection: "column", p: 6 }}>
        <Logo />
        <Box sx={{ mt: "auto", pb: 4 }}>
          <Typography variant="h2" sx={{ fontWeight: 700, letterSpacing: "-0.03em" }}>
            {COPY.brand}
          </Typography>
          <Typography variant="h6" color="text.secondary" sx={{ mt: 1.5 }}>
            {COPY.brandSub}
          </Typography>
        </Box>
      </Box>

      <Box sx={{ display: "flex", alignItems: "center", justifyContent: "center", px: 2.5, py: 6 }}>
        <Paper sx={{ width: "100%", maxWidth: 400, p: 3.5 }}>
          <Box sx={{ mb: 2, display: { lg: "none" } }}>
            <Logo />
          </Box>
          <Typography variant="h5" sx={{ fontWeight: 700 }}>
            {isRegister ? COPY.registerTitle : COPY.loginTitle}
          </Typography>
          <Stack component="form" spacing={2.25} sx={{ mt: 3 }} onSubmit={(ev) => void onSubmit(ev)} noValidate>
            <TextField
              label={COPY.account}
              name="username"
              autoComplete="username"
              spellCheck={false}
              slotProps={{ htmlInput: { maxLength: 32 } }}
              value={account}
              onChange={(e) => setAccount(e.target.value)}
              placeholder={COPY.accountPlaceholder}
              autoFocus
              error={Boolean(fieldErr.account)}
              helperText={fieldErr.account ?? COPY.accountHint}
              fullWidth
            />
            <TextField
              label={COPY.password}
              name="password"
              type={showPw ? "text" : "password"}
              autoComplete={isRegister ? "new-password" : "current-password"}
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              placeholder={COPY.passwordPlaceholder}
              error={Boolean(fieldErr.password)}
              helperText={fieldErr.password ?? (isRegister ? COPY.passwordHint : undefined)}
              fullWidth
              slotProps={{
                htmlInput: { maxLength: 128 },
                input: {
                  endAdornment: (
                    <InputAdornment position="end">
                      <IconButton
                        aria-label={showPw ? COPY.hidePassword : COPY.showPassword}
                        onClick={() => setShowPw((v) => !v)}
                        edge="end"
                      >
                        {showPw ? <VisibilityOff /> : <Visibility />}
                      </IconButton>
                    </InputAdornment>
                  ),
                },
              }}
            />
            {isRegister ? (
              <TextField
                label={COPY.confirmPassword}
                type={showPw ? "text" : "password"}
                autoComplete="new-password"
                slotProps={{ htmlInput: { maxLength: 128 } }}
                value={confirm}
                onChange={(e) => setConfirm(e.target.value)}
                placeholder={COPY.confirmPlaceholder}
                error={Boolean(fieldErr.confirm)}
                helperText={fieldErr.confirm}
                fullWidth
              />
            ) : null}
            {error ? (
              <Alert severity="error" role="alert">
                {error}
              </Alert>
            ) : null}
            <Button type="submit" variant="contained" disabled={pending} size="large" fullWidth>
              {pending ? <CircularProgress size={18} color="inherit" sx={{ mr: 1 }} /> : null}
              {pending
                ? isRegister
                  ? COPY.submittingRegister
                  : COPY.submittingLogin
                : isRegister
                  ? COPY.registerAction
                  : COPY.loginAction}
            </Button>
          </Stack>
          <Typography variant="body2" color="text.secondary" sx={{ mt: 2.5, textAlign: "center" }}>
            {isRegister ? COPY.hasAccount : COPY.noAccount}{" "}
            <Box
              component={RouterLink}
              to={isRegister ? "/login" : "/register"}
              sx={{ color: "primary.main", fontWeight: 600, textDecoration: "none" }}
            >
              {isRegister ? COPY.goLogin : COPY.goRegister}
            </Box>
          </Typography>
        </Paper>
      </Box>
    </Box>
  );
}
