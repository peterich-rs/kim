const DEFAULT_UPLOAD = "https://upload.kim.ainexc.com";

export type UploadedObject = {
  key: string;
  url: string;
  contentType: string;
  bytes: number;
};

/**
 * PUT-style raw upload. JWT from Royal. Bytes never go through WGateway.
 */
export async function uploadImage(
  token: string,
  body: Blob | ArrayBuffer | Uint8Array,
  opts: { origin?: string; contentType?: string } = {},
): Promise<UploadedObject> {
  const origin = (opts.origin ?? DEFAULT_UPLOAD).replace(/\/+$/, "");
  const contentType =
    opts.contentType ||
    (body instanceof Blob && body.type) ||
    "application/octet-stream";
  const resp = await fetch(`${origin}/v1/objects`, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${token}`,
      "Content-Type": contentType,
    },
    body: body as BodyInit,
  });
  if (!resp.ok) {
    const text = await resp.text();
    throw new Error(`upload ${resp.status}: ${text}`);
  }
  return (await resp.json()) as UploadedObject;
}
