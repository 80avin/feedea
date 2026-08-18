import { Button } from "@heroui/react";
import { ArrowPathIcon, CheckCircleIcon } from "@heroicons/react/24/outline";
import { applyUpdate, usePwa } from "../pwa/usePwa";

export default function UpdateBanner() {
  const { needRefresh, offlineReady } = usePwa();
  if (!needRefresh && !offlineReady) return null;
  return (
    <div className="fixed bottom-4 right-4 z-50 flex items-center gap-3 rounded-lg border border-app-border-strong bg-app-surface px-4 py-3 shadow-xl">
      <span className="text-sm text-app-text-2">
        {needRefresh ? (
          <span className="flex items-center gap-2">
            <ArrowPathIcon className="h-4 w-4 shrink-0" /> A new version is available.
          </span>
        ) : (
          <span className="flex items-center gap-2">
            <CheckCircleIcon className="h-4 w-4 shrink-0 text-emerald-500" /> Ready to work offline.
          </span>
        )}
      </span>
      {needRefresh && (
        <Button size="sm" variant="primary" onPress={applyUpdate}>
          Reload
        </Button>
      )}
    </div>
  );
}
