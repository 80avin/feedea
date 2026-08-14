export function formatAge(iso: string): string {
  const date = new Date(iso);
  const diffMs = Date.now() - date.getTime();
  if (Number.isNaN(diffMs) || diffMs < 0) {
    return date.toLocaleDateString();
  }
  const minutes = Math.floor(diffMs / 60_000);
  if (minutes < 60) {
    return `${Math.max(minutes, 1)}m`;
  }
  const hours = Math.floor(minutes / 60);
  if (hours < 24) {
    return `${hours}h`;
  }
  const days = Math.floor(hours / 24);
  if (days < 7) {
    return `${days}d`;
  }
  const weeks = Math.floor(days / 7);
  if (weeks < 52) {
    return `${weeks}w`;
  }
  return date.toLocaleDateString(undefined, { month: "short", day: "numeric", year: "numeric" });
}
