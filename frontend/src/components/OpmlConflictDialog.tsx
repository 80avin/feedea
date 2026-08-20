import { useMemo, useState } from "react";
import type { ReactNode } from "react";
import { Button, Modal, useOverlayState } from "@heroui/react";
import { useImportOpml } from "../state/hooks";
import type { ImportOpmlResponse, OpmlConflict, OpmlExistingFeed, OpmlResolution } from "../api/types";

export default function OpmlConflictDialog({
  open,
  opml,
  conflicts,
  onClose,
  onImported,
}: {
  open: boolean;
  opml: string;
  conflicts: OpmlConflict[];
  onClose: () => void;
  onImported: (result: ImportOpmlResponse) => void;
}) {
  const state = useOverlayState({ isOpen: open, onOpenChange: (isOpen) => { if (!isOpen) onClose(); } });
  const importOpml = useImportOpml();
  const [choices, setChoices] = useState<Record<number, OpmlResolution>>({});
  const [error, setError] = useState("");

  const defaultResolution = (conflict: OpmlConflict): OpmlResolution => {
    const existing = conflict.matches.find((m) => !m.id.startsWith("__file__:")) ?? conflict.matches[0];
    if (existing) {
      return { key: conflict.key, action: "keep-existing", keep_existing_feed_id: existing.id };
    }
    return { key: conflict.key, action: "keep-new" };
  };

  function fieldChanged(a: string, b: string): boolean {
    const na = a.trim().toLowerCase();
    const nb = b.trim().toLowerCase();
    return na !== nb && na !== "" && nb !== "";
  }

  function categoryLabel(category: string[]): string {
    return category.length > 0 ? category.join(" / ") : "Uncategorized";
  }

  function categoryChanged(a: string[], b: string[]): boolean {
    if (a.length !== b.length) return true;
    return a.some((seg, i) => fieldChanged(seg, b[i]));
  }

  function changedClass(changed: boolean): string {
    return changed
      ? "rounded bg-amber-500/10 px-1 text-amber-700 dark:text-amber-400"
      : "";
  }

  const initialChoices = useMemo(
    () => Object.fromEntries(conflicts.map((c) => [c.key, defaultResolution(c)])),
    [open, conflicts],
  );

  const choiceFor = (conflict: OpmlConflict): OpmlResolution =>
    choices[conflict.key] ?? initialChoices[conflict.key] ?? defaultResolution(conflict);

  const selectAllNew = () => {
    setChoices(Object.fromEntries(conflicts.map((c) => [c.key, { key: c.key, action: "keep-new" }])));
  };

  const selectAllExisting = () => {
    setChoices(Object.fromEntries(conflicts.map((c) => [c.key, defaultResolution(c)])));
  };

  const selectedMatch = (conflict: OpmlConflict): OpmlExistingFeed | undefined => {
    const choice = choiceFor(conflict);
    if (choice.action === "keep-existing") {
      return conflict.matches.find((m) => m.id === choice.keep_existing_feed_id);
    }
    // No specific feed selected (keep-new/skip): highlight the file's version against
    // the primary existing match so the New column isn't compared against "nothing".
    return (
      conflict.matches.find((m) => !m.id.startsWith("__file__:")) ?? conflict.matches[0]
    );
  };

  const submit = async () => {
    setError("");
    const resolutions = conflicts.map((c) => choiceFor(c));
    try {
      const result = await importOpml.mutateAsync({ opml, resolutions });
      onImported(result);
      onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Import failed");
    }
  };

  const kindLabel: Record<OpmlConflict["kind"], string> = {
    "same-feed": "Same feed, different details",
    "intra-file": "Duplicate within the file",
  };

  return (
    <Modal state={state}>
      <Modal.Backdrop>
        <Modal.Container size="cover" className="max-w-4xl">
          <Modal.Dialog>
            <Modal.Header>
              <Modal.Heading>Resolve duplicate feeds</Modal.Heading>
            </Modal.Header>
            <Modal.Body>
              <p className="text-sm text-app-text-muted">
                The imported file has {conflicts.length} feed{conflicts.length > 1 ? "s" : ""} that match existing sources. Choose which to keep.
              </p>
              <div className="mt-3 flex flex-wrap items-center gap-2">
                <Button size="sm" variant="secondary" onPress={selectAllNew} isDisabled={importOpml.isPending}>
                  Select all new
                </Button>
                <Button size="sm" variant="secondary" onPress={selectAllExisting} isDisabled={importOpml.isPending}>
                  Select all existing
                </Button>
              </div>
              <div className="mt-4 flex flex-col gap-4">
                {conflicts.map((conflict) => {
                  const choice = choiceFor(conflict);
                  return (
                    <div key={conflict.key} className="rounded-lg border border-app-border p-3">
                      <div className="flex items-center justify-between gap-2">
                        <p className="text-xs font-medium uppercase tracking-wider text-app-text-faint">{kindLabel[conflict.kind]}</p>
                        <span className="flex shrink-0 items-center gap-1">
                          {conflict.matches.map((match) => (
                            <BulkChoiceButton
                              key={match.id}
                              active={choice.action === "keep-existing" && choice.keep_existing_feed_id === match.id}
                              onClick={() =>
                                setChoices((prev) => ({ ...prev, [conflict.key]: { key: conflict.key, action: "keep-existing", keep_existing_feed_id: match.id } }))
                              }
                            >
                              {match.id.startsWith("__file__:") ? "Keep first" : "Keep existing"}
                            </BulkChoiceButton>
                          ))}
                          <BulkChoiceButton
                            active={choice.action === "keep-new"}
                            onClick={() => setChoices((prev) => ({ ...prev, [conflict.key]: { key: conflict.key, action: "keep-new" } }))}
                          >
                            Keep new
                          </BulkChoiceButton>
                          <BulkChoiceButton
                            active={choice.action === "skip"}
                            onClick={() => setChoices((prev) => ({ ...prev, [conflict.key]: { key: conflict.key, action: "skip" } }))}
                          >
                            Skip
                          </BulkChoiceButton>
                        </span>
                      </div>

                      <p className="mt-2 truncate text-xs text-app-text-muted">{conflict.opml.url}</p>
                      {(conflict.occurrences ?? 1) > 1 && (
                        <p className="mt-1 text-[11px] font-medium text-amber-600 dark:text-amber-400">
                          This source appears {conflict.occurrences}× in the file
                        </p>
                      )}

                      <div className="mt-3 grid grid-cols-2 gap-3">
                        <div className="min-w-0 rounded-md border border-app-border bg-app-surface/60 p-3">
                          <p className="text-xs font-semibold uppercase tracking-wider text-app-text-faint">Old</p>
                          {conflict.matches
                            .filter((m) => !m.id.startsWith("__file__:") || conflict.matches.every((x) => x.id.startsWith("__file__:")))
                            .map((match) => {
                              const isSelected = choice.action === "keep-existing" && choice.keep_existing_feed_id === match.id;
                              return (
                                <label key={match.id} className={`mt-2 flex cursor-pointer items-start gap-2 rounded p-1 text-sm ${isSelected ? "bg-app-selected/60" : ""}`}>
                                  <input
                                    type="radio"
                                    name={`conflict-${conflict.key}`}
                                    checked={isSelected}
                                    onChange={() =>
                                      setChoices((prev) => ({ ...prev, [conflict.key]: { key: conflict.key, action: "keep-existing", keep_existing_feed_id: match.id } }))
                                    }
                                    className="mt-0.5"
                                  />
                                  <span className="min-w-0 flex-1">
                                    <span className={`block ${changedClass(fieldChanged(match.title, conflict.opml.title))}`}>{match.title}</span>
                                    <span className={`block text-xs ${changedClass(categoryChanged(match.category, conflict.opml.category))}`}>{categoryLabel(match.category)}</span>
                                    {fieldChanged(match.url ?? "", conflict.opml.url) && (
                                      <span className="block truncate text-xs text-app-text-faint">{match.url}</span>
                                    )}
                                  </span>
                                </label>
                              );
                            })}
                        </div>
                        <div className="min-w-0 rounded-md border border-app-border bg-app-surface/60 p-3">
                          <p className="text-xs font-semibold uppercase tracking-wider text-app-text-faint">New</p>
                          <label className="mt-2 flex cursor-pointer items-start gap-2 rounded p-1 text-sm">
                            <input
                              type="radio"
                              name={`conflict-${conflict.key}`}
                              checked={choice.action === "keep-new"}
                              onChange={() => setChoices((prev) => ({ ...prev, [conflict.key]: { key: conflict.key, action: "keep-new" } }))}
                              className="mt-0.5"
                            />
                            <span className="min-w-0 flex-1">
                              <span className={`block ${changedClass(fieldChanged(selectedMatch(conflict)?.title ?? "", conflict.opml.title))}`}>{conflict.opml.title}</span>
                              <span className={`block text-xs ${changedClass(categoryChanged(selectedMatch(conflict)?.category ?? [], conflict.opml.category))}`}>{categoryLabel(conflict.opml.category)}</span>
                              {fieldChanged(selectedMatch(conflict)?.url ?? "", conflict.opml.url) && (
                                <span className="block truncate text-xs text-app-text-faint">{conflict.opml.url}</span>
                              )}
                            </span>
                          </label>
                        </div>
                      </div>
                    </div>
                  );
                })}
              </div>
              {error && <p className="mt-3 text-sm text-red-600 dark:text-red-400">{error}</p>}
            </Modal.Body>
            <Modal.Footer>
              <Button variant="ghost" size="sm" onPress={onClose} isDisabled={importOpml.isPending}>
                Cancel
              </Button>
              <Button variant="primary" size="sm" onPress={submit} isDisabled={importOpml.isPending}>
                {importOpml.isPending ? "Importing…" : "Apply"}
              </Button>
            </Modal.Footer>
          </Modal.Dialog>
        </Modal.Container>
      </Modal.Backdrop>
    </Modal>
  );
}

function BulkChoiceButton({
  active,
  onClick,
  children,
}: {
  active: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`rounded-full border px-2 py-0.5 text-xs font-medium transition-colors ${
        active
          ? "border-accent bg-accent-soft text-accent-soft-foreground"
          : "border-app-border text-app-text-muted hover:bg-app-hover/60 hover:text-app-text"
      }`}
    >
      {children}
    </button>
  );
}
