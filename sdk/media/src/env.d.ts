interface Env {
  BUCKET: R2Bucket;
  APP: string;
  PUBLIC_BASE: string;
  ROYAL_ORIGIN: string;
  MAX_BYTES: string;
  JWT_SECRET?: string;
}
