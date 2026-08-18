import { useMemo, useState } from "react";
import { Button, Modal, useOverlayState } from "@heroui/react";
import { useImportOpml } from "../state/hooks";
import type { ImportOpmlResponse, OpmlConflict, OpmlResolution } from "../api/types";

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

  const initialChoices = useMemo(
    () => Object.fromEntries(conflicts.map((c) => [c.key, defaultResolution(c)])),
    [open, conflicts],
  );

  const choiceFor = (conflict: OpmlConflict): OpmlResolution =>
    choices[conflict.key] ?? initialChoices[conflict.key] ?? defaultResolution(conflict);

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
    "url-identical": "Same feed URL, different details",
    "url-variant": "Same feed, different URL",
    "intra-file": "Duplicate within the file",
  };

  return (
    <Modal state={state}>
      <Modal.Backdrop>
        <Modal.Container>
          <Modal.Dialog>
            <Modal.Header>
              <Modal.Heading>Resolve duplicate feeds</Modal.Heading>
            </Modal.Header>
            <Modal.Body>
              <p className="text-sm text-app-text-muted">
                The imported file has {conflicts.length} feed{conflicts.length > 1 ? "s" : ""} that match existing sources. Choose which to keep.
              </p>
              <div className="mt-4 flex flex-col gap-4">
                {conflicts.map((conflict) => {
                  const choice = choiceFor(conflict);
                  return (
                    <div key={conflict.key} className="rounded-lg border border-app-border p-3">
                      <p className="text-xs font-medium uppercase tracking-wider text-app-text-faint">{kindLabel[conflict.kind]}</p>
                      <div className="mt-2 flex items-start gap-3">
                        <div className="min-w-0 flex-1">
                          <p className="truncate text-sm font-semibold text-app-text">{conflict.opml.title}</p>
                          <p className="truncate text-xs text-app-text-muted">{conflict.opml.url}</p>
                          <p className="truncate text-xs text-app-text-faint">{conflict.opml.category || "Uncategorized"}</p>
                        </div>
                        <div className="flex shrink-0 flex-col gap-1">
                          {conflict.matches.filter((m) => !m.id.startsWith("__file__:")).map((match) => (
                            <label key={match.id} className="flex items-center gap-2 text-sm text-app-text-2">
                              <input
                                type="radio"
                                name={`conflict-${conflict.key}`}
                                checked={choice.action === "keep-existing" && choice.keep_existing_feed_id === match.id}
                                onChange={() =>
                                  setChoices((prev) => ({ ...prev, [conflict.key]: { key: conflict.key, action: "keep-existing", keep_existing_feed_id: match.id } }))
                                }
                              />
                              <span className="min-w-0">
                                <span className="block truncate">{match.title}</span>
                                <span className="block truncate text-xs text-app-text-faint">{match.url ?? ""}</span>
                              </span>
                            </label>
                          ))}
                          <label className="flex items-center gap-2 text-sm text-app-text-2">
                            <input
                              type="radio"
                              name={`conflict-${conflict.key}`}
                              checked={choice.action === "keep-new"}
                              onChange={() =>
                                setChoices((prev) => ({ ...prev, [conflict.key]: { key: conflict.key, action: "keep-new" } }))
                              }
                            />
                            <span>
                              <span className="block">Keep new</span>
                              <span className="block truncate text-xs text-app-text-faint">{conflict.opml.url}</span>
                            </span>
                          </label>
                          <label className="flex items-center gap-2 text-sm text-app-text-2">
                            <input
                              type="radio"
                              name={`conflict-${conflict.key}`}
                              checked={choice.action === "skip"}
                              onChange={() =>
                                setChoices((prev) => ({ ...prev, [conflict.key]: { key: conflict.key, action: "skip" } }))
                              }
                            />
                            <span>Skip</span>
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