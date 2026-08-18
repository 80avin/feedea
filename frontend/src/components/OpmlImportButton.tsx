import { useRef, useState } from "react";
import { ArrowUpTrayIcon } from "@heroicons/react/24/outline";
import { useImportOpml } from "../state/hooks";
import { formatError } from "./Feedback";
import type { ImportOpmlResponse, OpmlConflict } from "../api/types";
import OpmlConflictDialog from "./OpmlConflictDialog";

function summaryText(result: ImportOpmlResponse): string {
  const migrated = (result.migrated ?? 0) > 0 ? `, migrated ${result.migrated}` : "";
  return `Imported ${result.added} feed(s), skipped ${result.skipped} duplicate(s)${migrated}`;
}

export default function OpmlImportButton({ className }: { className?: string }) {
  const importOpml = useImportOpml();
  const fileRef = useRef<HTMLInputElement>(null);
  const [status, setStatus] = useState("");
  const [conflictState, setConflictState] = useState<{ opml: string; conflicts: OpmlConflict[] } | null>(null);

  const onFile = async (file: File | null) => {
    if (!file) return;
    setStatus("");
    try {
      const text = await file.text();
      const result = await importOpml.mutateAsync({ opml: text });
      if (result.status === "conflicts") {
        setConflictState({ opml: text, conflicts: result.conflicts ?? [] });
      } else {
        setStatus(summaryText(result));
      }
    } catch (e) {
      setStatus(formatError(e));
    }
  };

  return (
    <span className={`relative ${className ?? ""}`}>
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
      <button
        type="button"
        aria-label="Import OPML"
        title="Import OPML"
        onClick={() => fileRef.current?.click()}
        disabled={importOpml.isPending}
        className="flex items-center justify-center rounded-md p-1.5 text-app-text-muted transition-colors hover:bg-app-hover/60 hover:text-app-text disabled:cursor-not-allowed disabled:opacity-50"
      >
        <ArrowUpTrayIcon className="h-4 w-4" />
      </button>
      {status && (
        <span className="absolute right-0 top-full z-10 mt-1 max-w-48 truncate rounded-md border border-app-border bg-app-bg px-2 py-1 text-xs text-app-text-faint shadow-lg">
          {status}
        </span>
      )}
      {conflictState && (
        <OpmlConflictDialog
          open
          opml={conflictState.opml}
          conflicts={conflictState.conflicts}
          onClose={() => setConflictState(null)}
          onImported={(result) => setStatus(summaryText(result))}
        />
      )}
    </span>
  );
}