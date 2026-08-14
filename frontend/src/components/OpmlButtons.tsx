import { useRef, useState } from "react";
import { Button } from "@heroui/react";
import { ArrowDownTrayIcon, ArrowUpTrayIcon } from "@heroicons/react/24/outline";
import { useExportOpml, useImportOpml } from "../state/hooks";
import { formatError } from "./Feedback";

export default function OpmlButtons() {
  const importOpml = useImportOpml();
  const exportOpml = useExportOpml();
  const fileRef = useRef<HTMLInputElement>(null);
  const [status, setStatus] = useState("");

  const onFile = async (file: File | null) => {
    if (!file) return;
    setStatus("");
    try {
      const text = await file.text();
      await importOpml.mutateAsync({ opml: text });
      setStatus(`Imported ${file.name}`);
    } catch (e) {
      setStatus(formatError(e));
    }
  };

  const onExport = async () => {
    setStatus("");
    try {
      const xml = await exportOpml.mutateAsync();
      const blob = new Blob([xml], { type: "text/xml" });
      const url = URL.createObjectURL(blob);
      const link = document.createElement("a");
      link.href = url;
      link.download = "rssea-subscriptions.opml";
      document.body.appendChild(link);
      link.click();
      link.remove();
      URL.revokeObjectURL(url);
      setStatus("Exported");
    } catch (e) {
      setStatus(formatError(e));
    }
  };

  return (
    <div className="relative flex flex-wrap items-center gap-2">
      <input
        ref={fileRef}
        type="file"
        accept=".opml,.xml,text/xml,application/xml"
        className="hidden"
        onChange={(e) => {
          void onFile(e.target.files?.[0] ?? null);
          e.target.value = "";
        }}
      />
      <Button size="sm" variant="secondary" onPress={() => fileRef.current?.click()} isDisabled={importOpml.isPending}>
        <ArrowUpTrayIcon className="h-4 w-4" />
        Import
      </Button>
      <Button size="sm" variant="secondary" onPress={onExport} isDisabled={exportOpml.isPending}>
        <ArrowDownTrayIcon className="h-4 w-4" />
        Export
      </Button>
      {status && (
        <span className="absolute right-0 top-full z-10 mt-1 max-w-48 truncate rounded-md border border-app-border bg-app-bg px-2 py-1 text-xs text-app-text-faint shadow-lg">
          {status}
        </span>
      )}
    </div>
  );
}
