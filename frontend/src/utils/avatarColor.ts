const STORAGE_KEY = "feedea.avatar-colors";

export const AVATAR_COLORS: string[] = [
  "oklch(0.546 0.245 262.881)",
  "oklch(0.627 0.194 149.214)",
  "oklch(0.541 0.281 293.009)",
  "oklch(0.681 0.162 75.834)",
  "oklch(0.645 0.246 16.439)",
  "oklch(0.588 0.158 241.966)",
  "oklch(0.600 0.118 184.704)",
  "oklch(0.667 0.295 322.15)",
];

function readMap(): Record<string, string> {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw ? (JSON.parse(raw) as Record<string, string>) : {};
  } catch {
    return {};
  }
}

function writeMap(map: Record<string, string>) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(map));
  } catch {}
}

function hashStr(input: string): number {
  let hash = 0;
  for (let i = 0; i < input.length; i++) {
    hash = (hash * 31 + input.charCodeAt(i)) | 0;
  }
  return Math.abs(hash);
}

export function avatarColorFor(feedId: string): string {
  const override = readMap()[feedId];
  if (override && AVATAR_COLORS.includes(override)) {
    return override;
  }
  return AVATAR_COLORS[hashStr(feedId) % AVATAR_COLORS.length];
}

export function avatarColorOverride(feedId: string): string | null {
  const override = readMap()[feedId];
  return override && AVATAR_COLORS.includes(override) ? override : null;
}

export function setAvatarColorOverride(feedId: string, color: string | null) {
  const map = readMap();
  if (color) {
    map[feedId] = color;
  } else {
    delete map[feedId];
  }
  writeMap(map);
}
