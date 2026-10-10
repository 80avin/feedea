import { useEffect } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { api, encodeId } from "../api/client";
import type { ArticleDetail, Headline } from "../api/types";
import { queryKeys } from "../state/keys";
import { useSettings } from "../state/hooks";

const MAX_WARM = 250;

async function warmDetails(queryClient: ReturnType<typeof useQueryClient>, ids: string[]) {
  const limited = [...new Set(ids)].slice(0, MAX_WARM);
  for (let i = 0; i < limited.length; i += 20) {
    const chunk = limited.slice(i, i + 20);
    await Promise.allSettled(
      chunk.map((id) =>
        queryClient.fetchQuery({
          queryKey: queryKeys.article(id),
          queryFn: () => api.get<ArticleDetail>(`/api/articles/${encodeId(id)}`),
          staleTime: 5 * 60_000,
        }),
      ),
    );
  }
}

export function useOfflineWarmer(overviewIds: string[]) {
  const queryClient = useQueryClient();
  const { data: settings } = useSettings();
  useEffect(() => {
    if (!settings) return;
    (async () => {
      try {
        const ov = settings.offline_cache_overview ? [...new Set(overviewIds)].slice(0, MAX_WARM) : [];
        if (ov.length > 0) {
          await warmDetails(queryClient, ov);
        }
        if (settings.offline_cache_recent) {
          const n = Math.min(Math.max(settings.offline_recent_count || 50, 1), 200);
          const recent = await api.get<Headline[]>(`/api/articles?limit=${n}`);
          const fresh = recent.map((h) => h.id).filter((id) => !overviewIds.includes(id));
          const budgeted = fresh.slice(0, Math.max(0, MAX_WARM - ov.length));
          if (budgeted.length > 0) await warmDetails(queryClient, budgeted);
        }
      } catch {
        // Offline or warming failure — never surface to UI.
      }
    })();
  }, [queryClient, settings, overviewIds.join("|")]);
}
