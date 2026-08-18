import { useLocation } from "react-router";
import { articlePath } from "../utils/articleLink";

export function useArticlePath() {
  const location = useLocation();
  return (id: string) => articlePath(id, new URLSearchParams(location.search));
}
