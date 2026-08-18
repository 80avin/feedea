import { encodeId } from "../api/client";

export function articlePath(id: string, search: URLSearchParams): string {
  const next = new URLSearchParams(search);
  next.delete("article");
  const query = next.toString();
  return `/feeds/${encodeId(id)}${query ? `?${query}` : ""}`;
}
