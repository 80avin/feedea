import { useEffect } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { api, encodeId } from "../api/client";
import type { ArticleDetail, Headline } from "../api/types";
import { queryKeys } from "../state/keys";
import { useSettings } from "../state/hooks";

const MAX_WARM = 250;

async function warmDetails(queryClient: ReturnType<typeof useQueryClient>, ids: string[]) {
  const limited = [...new Set(ids)].slice(0, MAX_WARM);
  await Promise.allSettled(
    limited.map((id) =>
      queryClient.fetchQuery({
        queryKey: queryKeys.article(id),
        queryFn: () => api.get<ArticleDetail>(`/api/articles/${encodeId(id)}`),
        staleTime: 5 * 60_000,
      }),
    ),
  );
}

export function useOfflineWarmer(overviewIds: string[]) {
  const queryClient = useQueryClient();
  const { data: settings } = useSettings();
  useEffect(() => {
    if (!settings) return;
    (async () => {
      try {
        if (settings.offline_cache_overview && overviewIds.length > 0) {
          await warmDetails(queryClient, overviewIds);
        }
        if (settings.offline_cache_recent) {
          const n = Math.min(Math.max(settings.offline_recent_count || 50, 1), 200);
          const recent = await api.get<Headline[]>(`/api/articles?limit=${n}`);
          const fresh = recent.map((h) => h.id).filter((id) => !overviewIds.includes(id));
          if (fresh.length > 0) await warmDetails(queryClient, fresh);
        }
      } catch {
        // Offline or warming failure — never surface to UI.
      }
    })();
  }, [queryClient, settings, overviewIds.join("|")]);
}
