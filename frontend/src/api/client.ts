import type { ErrorEnvelope } from "./types";

export class ApiError extends Error {
  status: number;
  code: string;

  constructor(status: number, code: string, message: string) {
    super(message);
    this.name = "ApiError";
    this.status = status;
    this.code = code;
  }
}

const AUTH_EXEMPT = new Set(["/api/login", "/api/session"]);

export async function apiFetch<T>(path: string, init: RequestInit = {}): Promise<T> {
  const res = await fetch(path, init);
  if (res.ok) {
    if (res.status === 204) return undefined as T;
    return (await res.json()) as T;
  }
  let code = "";
  let message = "";
  try {
    const body = (await res.json()) as ErrorEnvelope;
    code = body.error?.code ?? "";
    message = body.error?.message ?? "";
  } catch {
    message = res.statusText;
  }
  if (res.status === 401 && !AUTH_EXEMPT.has(path)) {
    window.dispatchEvent(new Event("rssea:unauthorized"));
  }
  throw new ApiError(res.status, code, message);
}

export function apiGet<T>(path: string): Promise<T> {
  return apiFetch<T>(path);
}

function apiWithBody(method: string) {
  return function apiBody<T>(path: string, body?: unknown): Promise<T> {
    return apiFetch<T>(path, {
      method,
      headers: body !== undefined ? { "Content-Type": "application/json" } : undefined,
      body: body !== undefined ? JSON.stringify(body) : undefined,
    });
  };
}

export function apiPost<T>(path: string, body?: unknown): Promise<T> {
  return apiWithBody("POST")(path, body);
}

export function apiPatch<T>(path: string, body?: unknown): Promise<T> {
  return apiWithBody("PATCH")(path, body);
}

export function apiDelete<T>(path: string): Promise<T> {
  return apiFetch<T>(path, { method: "DELETE" });
}

export const api = {
  get: apiGet,
  post: apiPost,
  patch: apiPatch,
  delete: apiDelete,
};
