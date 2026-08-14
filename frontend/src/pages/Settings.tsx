import { useEffect, useState } from "react";
import { Button } from "@heroui/react";
import { ArrowRightStartOnRectangleIcon, Cog6ToothIcon } from "@heroicons/react/24/outline";
import { useSession } from "../auth/useSession";
import { useChangePassword, useSettings, useUpdateSettings } from "../state/hooks";
import { ErrorState, LoadingState, formatError } from "../components/Feedback";

function Field({ label, hint, children }: { label: string; hint?: string; children: React.ReactNode }) {
  return (
    <label className="flex flex-col gap-1.5">
      <span className="text-sm font-medium text-zinc-300">{label}</span>
      {children}
      {hint && <span className="text-xs text-zinc-500">{hint}</span>}
    </label>
  );
}

export default function Settings() {
  const { logout } = useSession();
  const { data, isLoading, isError, error, refetch } = useSettings();
  const updateSettings = useUpdateSettings();
  const changePassword = useChangePassword();

  const [theme, setTheme] = useState("light");
  const [syncInterval, setSyncInterval] = useState("30");
  const [keepDays, setKeepDays] = useState("");
  const [savedBanner, setSavedBanner] = useState("");

  const [currentPassword, setCurrentPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [passwordBanner, setPasswordBanner] = useState("");

  useEffect(() => {
    if (data) {
      setTheme(data.theme ?? "light");
      setSyncInterval(String(data.sync_interval_minutes));
      setKeepDays(data.keep_articles_days === null ? "" : String(data.keep_articles_days));
    }
  }, [data]);

  const savePreferences = async () => {
    const patch: { theme?: string; sync_interval_minutes?: number; keep_articles_days?: number | null } = {};
    if (theme !== (data?.theme ?? "light")) {
      patch.theme = theme;
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
        <Cog6ToothIcon className="h-5 w-5 text-zinc-400" />
        <h2 className="text-lg font-semibold">Settings</h2>
      </div>

      {isLoading && <div className="mt-4"><LoadingState /></div>}
      {isError && <div className="mt-4"><ErrorState error={error} onRetry={() => refetch()} /></div>}
      {data && (
        <div className="mt-4 flex-1 space-y-6 overflow-y-auto">
          <section className="flex flex-col gap-4 rounded-lg border border-zinc-800 p-4">
            <h3 className="text-sm font-semibold">Preferences</h3>
            <Field label="Theme">
              <select
                value={theme}
                onChange={(e) => setTheme(e.target.value)}
                className="rounded-md border border-zinc-700 bg-zinc-900 px-3 py-2 text-sm text-zinc-100 outline-none focus:border-zinc-500"
              >
                <option value="light">Light</option>
                <option value="dark">Dark</option>
                <option value="system">System</option>
              </select>
            </Field>
            <Field label="Sync interval (minutes)" hint="How often feeds are refreshed.">
              <input
                type="number"
                min={1}
                value={syncInterval}
                onChange={(e) => setSyncInterval(e.target.value)}
                className="rounded-md border border-zinc-700 bg-zinc-900 px-3 py-2 text-sm text-zinc-100 outline-none focus:border-zinc-500"
              />
            </Field>
            <Field label="Keep articles (days)" hint="Leave empty to keep articles forever.">
              <input
                type="number"
                min={1}
                value={keepDays}
                onChange={(e) => setKeepDays(e.target.value)}
                className="rounded-md border border-zinc-700 bg-zinc-900 px-3 py-2 text-sm text-zinc-100 outline-none focus:border-zinc-500"
              />
            </Field>
            <div className="flex items-center gap-3">
              <Button size="sm" variant="primary" isDisabled={updateSettings.isPending} onPress={savePreferences}>
                {updateSettings.isPending ? "Saving…" : "Save preferences"}
              </Button>
              {savedBanner && <span className="text-sm text-zinc-400">{savedBanner}</span>}
            </div>
          </section>

          <section className="flex flex-col gap-4 rounded-lg border border-zinc-800 p-4">
            <h3 className="text-sm font-semibold">Change password</h3>
            <Field label="Current password">
              <input
                type="password"
                value={currentPassword}
                onChange={(e) => setCurrentPassword(e.target.value)}
                className="rounded-md border border-zinc-700 bg-zinc-900 px-3 py-2 text-sm text-zinc-100 outline-none focus:border-zinc-500"
              />
            </Field>
            <Field label="New password">
              <input
                type="password"
                value={newPassword}
                onChange={(e) => setNewPassword(e.target.value)}
                className="rounded-md border border-zinc-700 bg-zinc-900 px-3 py-2 text-sm text-zinc-100 outline-none focus:border-zinc-500"
              />
            </Field>
            <Field label="Confirm new password">
              <input
                type="password"
                value={confirmPassword}
                onChange={(e) => setConfirmPassword(e.target.value)}
                className="rounded-md border border-zinc-700 bg-zinc-900 px-3 py-2 text-sm text-zinc-100 outline-none focus:border-zinc-500"
              />
            </Field>
            <div className="flex items-center gap-3">
              <Button size="sm" variant="primary" isDisabled={changePassword.isPending} onPress={savePassword}>
                {changePassword.isPending ? "Changing…" : "Change password"}
              </Button>
              {passwordBanner && <span className="text-sm text-zinc-400">{passwordBanner}</span>}
            </div>
          </section>

          {data.stats && (
            <section className="grid grid-cols-2 gap-3 rounded-lg border border-zinc-800 p-4 text-sm">
              <div>
                <p className="text-xs uppercase tracking-wider text-zinc-500">Feeds</p>
                <p className="font-semibold">{data.stats.feeds}</p>
              </div>
              <div>
                <p className="text-xs uppercase tracking-wider text-zinc-500">Articles</p>
                <p className="font-semibold">{data.stats.articles}</p>
              </div>
              <div>
                <p className="text-xs uppercase tracking-wider text-zinc-500">Unread</p>
                <p className="font-semibold">{data.stats.unread}</p>
              </div>
              <div>
                <p className="text-xs uppercase tracking-wider text-zinc-500">DB size</p>
                <p className="font-semibold">{(data.stats.database_size_bytes / 1024 / 1024).toFixed(1)} MB</p>
              </div>
              <div className="col-span-2">
                <p className="text-xs uppercase tracking-wider text-zinc-500">Last sync</p>
                <p className="font-semibold">{new Date(data.stats.last_sync).toLocaleString()}</p>
              </div>
            </section>
          )}

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
