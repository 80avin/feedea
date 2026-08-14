import { useMatch, useSearchParams } from "react-router";

export function useSelectedArticleId(): string | null {
  const match = useMatch("/feeds/*");
  const [searchParams] = useSearchParams();
  const pathId = match?.params["*"];
  if (pathId) {
    return pathId;
  }
  return searchParams.get("article") ?? null;
}
