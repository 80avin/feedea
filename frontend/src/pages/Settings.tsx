import { useEffect, useState } from "react";
import { Button } from "@heroui/react";
import { ArrowRightStartOnRectangleIcon, Cog6ToothIcon } from "@heroicons/react/24/outline";
import { useSession } from "../auth/useSession";
import { useChangePassword, useSettings, useUpdateSettings } from "../state/hooks";
import { ErrorState, LoadingState, formatError } from "../components/Feedback";
import { ACCENTS } from "../theme/useTheme";

const REPO_URL = "https://github.com/yourname/feedea";
const ISSUES_URL = "https://github.com/yourname/feedea/issues";

const inputClass =
  "rounded-md border border-app-border-strong bg-app-surface px-3 py-2 text-sm text-app-text outline-none focus:border-accent";
const selectClass =
  "rounded-md border border-app-border-strong bg-app-surface px-3 py-2 text-sm text-app-text outline-none focus:border-accent";

function Field({ label, hint, children }: { label: string; hint?: string; children: React.ReactNode }) {
  return (
    <label className="flex flex-col gap-1.5">
      <span className="text-sm font-medium text-app-text-2">{label}</span>
      {children}
      {hint && <span className="text-xs text-app-text-faint">{hint}</span>}
    </label>
  );
}

export default function Settings() {
  const { logout } = useSession();
  const { data, isLoading, isError, error, refetch } = useSettings();
  const updateSettings = useUpdateSettings();
  const changePassword = useChangePassword();

  const [theme, setTheme] = useState("dark");
  const [accent, setAccent] = useState("blue");
  const [syncInterval, setSyncInterval] = useState("30");
  const [keepDays, setKeepDays] = useState("");
  const [savedBanner, setSavedBanner] = useState("");
  const [dirty, setDirty] = useState(false);

  const markDirty = () => setDirty(true);

  const [currentPassword, setCurrentPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [passwordBanner, setPasswordBanner] = useState("");

  useEffect(() => {
    if (data && !dirty) {
      setTheme(data.theme ?? "dark");
      setAccent(data.accent ?? "blue");
      setSyncInterval(String(data.sync_interval_minutes));
      setKeepDays(data.keep_articles_days === null ? "" : String(data.keep_articles_days));
    }
  }, [data, dirty]);

  const savePreferences = async () => {
    const patch: { theme?: string; accent?: string; sync_interval_minutes?: number; keep_articles_days?: number | null } = {};
    if (theme !== (data?.theme ?? "dark")) {
      patch.theme = theme;
    }
    if (accent !== (data?.accent ?? "blue")) {
      patch.accent = accent;
    }
    const interval = Number(syncInterval);
    if (!Number.isNaN(interval) && interval > 0 && interval !== data?.sync_interval_minutes) {
      patch.sync_interval_minutes = interval;
    }
    const parsedDays = keepDays === "" ? null : Number(keepDays);
    if (keepDays === "" ? data?.keep_articles_days !== null : parsedDays !== data?.keep_articles_days) {
      if (parsedDays !== null && Number.isNaN(parsedDays)) {
        setSavedBanner("Keep-articles must be a number.");
        return;
      }
      patch.keep_articles_days = parsedDays;
    }
    if (Object.keys(patch).length === 0) {
      setSavedBanner("No changes to save.");
      return;
    }
    try {
      await updateSettings.mutateAsync(patch);
      setDirty(false);
      setSavedBanner("Settings saved.");
    } catch (e) {
      setSavedBanner(formatError(e));
    }
  };

  const savePassword = async () => {
    setPasswordBanner("");
    if (!currentPassword || !newPassword) {
      setPasswordBanner("Fill in both password fields.");
      return;
    }
    if (newPassword !== confirmPassword) {
      setPasswordBanner("New passwords do not match.");
      return;
    }
    try {
      await changePassword.mutateAsync({ current_password: currentPassword, new_password: newPassword });
      setCurrentPassword("");
      setNewPassword("");
      setConfirmPassword("");
      setPasswordBanner("Password changed.");
    } catch (e) {
      setPasswordBanner(formatError(e));
    }
  };

  return (
    <div className="flex h-full flex-col p-4">
      <div className="flex items-center gap-2">
        <Cog6ToothIcon className="h-5 w-5 text-app-text-muted" />
        <h2 className="text-lg font-semibold">Settings</h2>
      </div>

      {isLoading && <div className="mt-4"><LoadingState /></div>}
      {isError && <div className="mt-4"><ErrorState error={error} onRetry={() => refetch()} /></div>}
      {data && (
        <div className="mt-4 flex-1 space-y-6 overflow-y-auto">
          <section className="flex flex-col gap-4 rounded-lg border border-app-border p-4">
            <h3 className="text-sm font-semibold">Preferences</h3>
            <Field label="Theme">
              <select
                value={theme}
                onChange={(e) => {
                  setTheme(e.target.value);
                  markDirty();
                }}
                className={selectClass}
              >
                <option value="dark">Dark</option>
                <option value="light">Light</option>
                <option value="system">System</option>
              </select>
            </Field>
            <Field label="Accent color" hint="Used for buttons, active navigation and focus.">
              <select
                value={accent}
                onChange={(e) => {
                  setAccent(e.target.value);
                  markDirty();
                }}
                className={selectClass}
              >
                {ACCENTS.map((a) => (
                  <option key={a.id} value={a.id}>
                    {a.label}
                  </option>
                ))}
              </select>
            </Field>
            <Field label="Sync interval (minutes)" hint="How often feeds are refreshed.">
              <input
                type="number"
                min={1}
                value={syncInterval}
                onChange={(e) => {
                  setSyncInterval(e.target.value);
                  markDirty();
                }}
                className={inputClass}
              />
            </Field>
            <Field label="Keep articles (days)" hint="Leave empty to keep articles forever.">
              <input
                type="number"
                min={1}
                value={keepDays}
                onChange={(e) => {
                  setKeepDays(e.target.value);
                  markDirty();
                }}
                className={inputClass}
              />
            </Field>
            <div className="flex items-center gap-3">
              <Button size="sm" variant="primary" isDisabled={updateSettings.isPending} onPress={savePreferences}>
                {updateSettings.isPending ? "Saving…" : "Save preferences"}
              </Button>
              {savedBanner && <span className="text-sm text-app-text-muted">{savedBanner}</span>}
            </div>
          </section>

          <section className="flex flex-col gap-4 rounded-lg border border-app-border p-4">
            <h3 className="text-sm font-semibold">Change password</h3>
            <Field label="Current password">
              <input
                type="password"
                value={currentPassword}
                onChange={(e) => setCurrentPassword(e.target.value)}
                className={inputClass}
              />
            </Field>
            <Field label="New password">
              <input
                type="password"
                value={newPassword}
                onChange={(e) => setNewPassword(e.target.value)}
                className={inputClass}
              />
            </Field>
            <Field label="Confirm new password">
              <input
                type="password"
                value={confirmPassword}
                onChange={(e) => setConfirmPassword(e.target.value)}
                className={inputClass}
              />
            </Field>
            <div className="flex items-center gap-3">
              <Button size="sm" variant="primary" isDisabled={changePassword.isPending} onPress={savePassword}>
                {changePassword.isPending ? "Changing…" : "Change password"}
              </Button>
              {passwordBanner && <span className="text-sm text-app-text-muted">{passwordBanner}</span>}
            </div>
          </section>

          {data.stats && (
            <section className="grid grid-cols-2 gap-3 rounded-lg border border-app-border p-4 text-sm">
              <div>
                <p className="text-xs uppercase tracking-wider text-app-text-faint">Feeds</p>
                <p className="font-semibold">{data.stats.feeds}</p>
              </div>
              <div>
                <p className="text-xs uppercase tracking-wider text-app-text-faint">Articles</p>
                <p className="font-semibold">{data.stats.articles}</p>
              </div>
              <div>
                <p className="text-xs uppercase tracking-wider text-app-text-faint">Unread</p>
                <p className="font-semibold">{data.stats.unread}</p>
              </div>
              <div>
                <p className="text-xs uppercase tracking-wider text-app-text-faint">DB size</p>
                <p className="font-semibold">{(data.stats.database_size_bytes / 1024 / 1024).toFixed(1)} MB</p>
              </div>
              <div className="col-span-2">
                <p className="text-xs uppercase tracking-wider text-app-text-faint">Last sync</p>
                <p className="font-semibold">{new Date(data.stats.last_sync).toLocaleString()}</p>
              </div>
            </section>
          )}

          <section className="flex flex-col gap-3 rounded-lg border border-app-border p-4">
            <h3 className="text-sm font-semibold">About</h3>
            <p className="text-sm text-app-text-muted">
              feedea is a self-hosted RSS feed aggregator. The repository is not published
              yet; the links below will be updated once it goes public.
            </p>
            <div className="flex flex-wrap gap-2">
              <a
                href={REPO_URL}
                target="_blank"
                rel="noreferrer"
                className="rounded-md border border-app-border-strong px-3 py-1.5 text-sm text-app-text-2 hover:border-app-border hover:text-app-text"
              >
                Repository
              </a>
              <a
                href={ISSUES_URL}
                target="_blank"
                rel="noreferrer"
                className="rounded-md border border-app-border-strong px-3 py-1.5 text-sm text-app-text-2 hover:border-app-border hover:text-app-text"
              >
                Report an issue
              </a>
            </div>
          </section>

          <div className="flex-1" />
          <Button variant="ghost" size="sm" fullWidth onPress={logout} className="justify-start">
            <ArrowRightStartOnRectangleIcon className="h-4 w-4" />
            Log out
          </Button>
        </div>
      )}
    </div>
  );
}
