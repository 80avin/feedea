import { useEffect, useState } from "react";
import { Button, Radio, RadioGroup } from "@heroui/react";
import { ArrowRightStartOnRectangleIcon, CheckIcon, Cog6ToothIcon, TrashIcon } from "@heroicons/react/24/outline";
import { useSession } from "../auth/useSession";
import {
  useChangePassword,
  useDeleteEmptyCategories,
  useSettings,
  useUpdateSettings,
} from "../state/hooks";
import { ErrorState, LoadingState, formatError } from "../components/Feedback";
import { ACCENTS } from "../theme/useTheme";
import { ArrowLeftEndOnRectangleIcon, ComputerDesktopIcon, MoonIcon, SunIcon } from "@heroicons/react/24/solid";
import { installApp, usePwa } from "../pwa/usePwa";

const REPO_URL = "https://github.com/80avin/feedea";
const ISSUES_URL = "https://github.com/80avin/feedea/issues";

const inputClass =
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
  const deleteEmptyCategories = useDeleteEmptyCategories();
  const { installPrompt, isInstalled } = usePwa();

  const [theme, setTheme] = useState("system");
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
  const [adminBanner, setAdminBanner] = useState("");

  const deleteEmpty = async () => {
    setAdminBanner("");
    try {
      const res = await deleteEmptyCategories.mutateAsync();
      const count = res.deleted.length;
      setAdminBanner(
        count === 0
          ? "No empty categories found."
          : `Deleted ${count} empty categor${count === 1 ? "y" : "ies"}.`,
      );
    } catch (e) {
      setAdminBanner(formatError(e));
    }
  };

  useEffect(() => {
    if (data && !dirty) {
      setTheme(data.theme ?? "system");
      setAccent(data.accent ?? "blue");
      setSyncInterval(String(data.sync_interval_minutes));
      setKeepDays(data.keep_articles_days === null ? "" : String(data.keep_articles_days));
    }
  }, [data, dirty]);

  const savePreferences = async () => {
    const patch: { theme?: string; accent?: string; sync_interval_minutes?: number; keep_articles_days?: number | null } = {};
    if (theme !== (data?.theme ?? "system")) {
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
              <RadioGroup className="flex-row gap-2" value={theme}>
                {(
                  [
                    ["dark", MoonIcon],
                    ["light", SunIcon],
                    ["system", ComputerDesktopIcon],
                  ] as const
                ).map(([id, IconEl]) => (
                  <Radio key={id} value={id}>
                    <Radio.Content>
                      <Button
                        variant={theme === id ? "primary" : "outline"}
                        isIconOnly
                          onClick={() => {updateSettings.mutateAsync({theme: id})}}
                      >
                        <IconEl />
                      </Button>
                    </Radio.Content>
                  </Radio>
                ))}
              </RadioGroup>
            </Field>
            <Field label="Accent color" hint="Used for buttons, active navigation and focus.">
              <RadioGroup className="flex-row gap-2" value={accent}>
                {ACCENTS.map((ac) => (
                  <Radio key={ac.id} value={ac.id}>
                    {(e) => (
                      <Button
                        variant="tertiary"
                        isIconOnly
                        style={{
                          backgroundColor: ac.color,
                          outline: e.isSelected
                            ? "1px solid var(--accent-soft)"
                            : undefined,
                        }}
                        onClick={() => {
                          updateSettings.mutateAsync({ accent: ac.id });
                        }}
                      />
                    )}
                  </Radio>
                ))}
              </RadioGroup>
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
            <h3 className="text-sm font-semibold">Advanced</h3>
            <p className="text-xs text-app-text-faint">
              Administrative maintenance actions. These affect data immediately.
            </p>
            <div className="flex items-center gap-3">
              <Button
                size="sm"
                variant="ghost"
                onPress={deleteEmpty}
                isDisabled={deleteEmptyCategories.isPending}
              >
                <TrashIcon className="h-4 w-4" />
                {deleteEmptyCategories.isPending ? "Deleting…" : "Delete empty categories"}
              </Button>
              {adminBanner && <span className="text-sm text-app-text-muted">{adminBanner}</span>}
            </div>
          </section>

          <section className="flex flex-col gap-3 rounded-lg border border-app-border p-4">
            <p>Install to homescreen</p>
            {isInstalled ? (
              <p className="flex items-center gap-2 text-sm text-app-text-muted">
                <CheckIcon className="h-4 w-4 shrink-0 text-emerald-500" />
                App is installed.
              </p>
            ) : installPrompt ? (
              <Button onPress={installApp}>
                <ArrowLeftEndOnRectangleIcon className="h-4 w-4" />
                Install
              </Button>
            ) : (
              <>
                <p className="text-sm text-app-text-muted">App not installable as PWA.</p>
                <p className="text-xs font-medium uppercase tracking-wider text-app-text-faint">Possible reasons</p>
                <ul className="list-disc space-y-1.5 pl-5 text-sm text-app-text-muted">
                  <li>App already installed</li>
                  {window.location.protocol === "http:" && (
                    <li>
                      http: websites are not installable. If the host is private
                      and known to be secure, configure{" "}
                      <code>chrome://flags/#unsafely-treat-insecure-origin-as-secure</code>
                    </li>
                  )}
                  <li>On mobile, check if the launcher supports PWA installation.</li>
                </ul>
              </>
            )}
          </section>

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
