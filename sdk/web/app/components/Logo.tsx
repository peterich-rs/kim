import Box from "@mui/material/Box";
import Stack from "@mui/material/Stack";
import Typography from "@mui/material/Typography";

export function Logo() {
  return (
    <Stack direction="row" spacing={1.25} sx={{ alignItems: "center" }}>
      <Box
        component="svg"
        viewBox="0 0 32 32"
        sx={{ width: 32, height: 32, flexShrink: 0 }}
        aria-hidden="true"
      >
        <rect width="32" height="32" rx="8" fill="#0f766e" />
        <path fill="#ffffff" d="M9 8h4.2l4.1 6.6V8H22v16h-4.2l-4.1-6.6V24H9z" />
      </Box>
      <Typography variant="subtitle1" sx={{ fontWeight: 700, letterSpacing: "-0.02em" }}>
        KIM
      </Typography>
    </Stack>
  );
}
