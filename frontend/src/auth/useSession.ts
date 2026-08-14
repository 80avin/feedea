import { useCallback, useEffect } from "react";
import { useLocation, useNavigate } from "react-router";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../api/client";
import type { SessionInfo } from "../api/types";

const SESSION_QUERY_KEY = ["session"] as const;

export function useSession() {
  const navigate = useNavigate();
  const location = useLocation();
  const queryClient = useQueryClient();

  const { data: session, isLoading, isError, error, refetch } = useQuery({
    queryKey: SESSION_QUERY_KEY,
    queryFn: () => api.get<SessionInfo>("/api/session"),
    retry: false,
  });

  useEffect(() => {
    if (!isLoading && session && !session.authenticated && location.pathname !== "/login") {
      navigate("/login", { replace: true });
    }
  }, [isLoading, session, location.pathname, navigate]);

  useEffect(() => {
    const onUnauthorized = () => {
      queryClient.setQueryData(SESSION_QUERY_KEY, (prev?: SessionInfo) =>
        prev ? { ...prev, authenticated: false } : prev,
      );
      if (location.pathname !== "/login") {
        navigate("/login", { replace: true });
      }
    };
    window.addEventListener("rssea:unauthorized", onUnauthorized);
    return () => window.removeEventListener("rssea:unauthorized", onUnauthorized);
  }, [location.pathname, navigate, queryClient]);

  const refresh = useCallback(async () => {
    await queryClient.invalidateQueries({ queryKey: SESSION_QUERY_KEY });
  }, [queryClient]);

  const login = useCallback(
    async (password: string) => {
      await api.post("/api/login", { password });
      await queryClient.invalidateQueries({ queryKey: SESSION_QUERY_KEY });
    },
    [queryClient],
  );

  const logout = useCallback(async () => {
    try {
      await api.post("/api/logout");
    } finally {
      queryClient.setQueryData(SESSION_QUERY_KEY, {
        authenticated: false,
        version: "",
        setup_required: false,
      } satisfies SessionInfo);
      navigate("/login", { replace: true });
    }
  }, [navigate, queryClient]);

  return { session, loading: isLoading, isError, error, login, logout, refresh, retry: refetch };
}
