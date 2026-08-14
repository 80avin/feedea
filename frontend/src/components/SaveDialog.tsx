import { useEffect, useState } from "react";
import { Button, Chip, Input, Modal, TextArea, useOverlayState } from "@heroui/react";
import type { ArticleDetail } from "../api/types";
import { useSaveArticle, useTags, useUpdateNoteTags } from "../state/hooks";
import { formatError } from "./Feedback";

interface SaveDialogProps {
  open: boolean;
  article: ArticleDetail;
  onClose: () => void;
}

function parseTags(text: string): string[] {
  return [...new Set(text.split(",").map((t) => t.trim()).filter(Boolean))];
}

export default function SaveDialog({ open, article, onClose }: SaveDialogProps) {
  const state = useOverlayState({
    isOpen: open,
    onOpenChange: (isOpen) => {
      if (!isOpen) onClose();
    },
  });
  const [note, setNote] = useState("");
  const [tagsText, setTagsText] = useState("");
  const [error, setError] = useState("");
  const create = useSaveArticle();
  const update = useUpdateNoteTags();
  const { data: tagsData } = useTags();
  const suggestions = tagsData?.tags ?? [];
  const pending = create.isPending || update.isPending;

  useEffect(() => {
    if (open) {
      setNote(article.note ?? "");
      setTagsText(article.tags.join(", "));
      setError("");
    }
  }, [open, article]);

  const applySuggestion = (tag: string) => {
    setTagsText((prev) => {
      const current = parseTags(prev);
      return current.includes(tag) ? prev : [...current, tag].join(", ");
    });
  };

  const submit = async () => {
    const tags = parseTags(tagsText);
    const payload = { id: article.id, note: note.trim() || undefined, tags };
    try {
      await (article.marked ? update : create).mutateAsync(payload);
      onClose();
    } catch (e) {
      setError(formatError(e));
    }
  };

  return (
    <Modal state={state}>
      <Modal.Backdrop>
        <Modal.Container>
          <Modal.Dialog>
            <Modal.Header>
              <Modal.Heading>{article.marked ? "Edit note & tags" : "Save article"}</Modal.Heading>
            </Modal.Header>
            <Modal.Body>
              <label className="flex flex-col gap-1.5">
                <span className="text-sm font-medium text-zinc-300">Note</span>
                <TextArea
                  name="note"
                  value={note}
                  onChange={(e) => setNote(e.target.value)}
                  placeholder="Add a note (optional)"
                />
              </label>
              <label className="flex flex-col gap-1.5">
                <span className="text-sm font-medium text-zinc-300">Tags</span>
                <Input
                  name="tags"
                  value={tagsText}
                  onChange={(e) => setTagsText(e.target.value)}
                  placeholder="comma-separated tags"
                  aria-label="tags"
                />
              </label>
              {suggestions.length > 0 && (
                <div className="flex flex-wrap items-center gap-1.5">
                  <span className="text-xs text-zinc-500">Suggestions:</span>
                  {suggestions.map((tag) => (
                    <Chip key={tag} size="sm" variant="soft" onClick={() => applySuggestion(tag)}>
                      {tag}
                    </Chip>
                  ))}
                </div>
              )}
              {error && <p className="text-sm text-red-400">{error}</p>}
            </Modal.Body>
            <Modal.Footer>
              <Button variant="ghost" size="sm" onPress={onClose} isDisabled={pending}>
                Cancel
              </Button>
              <Button variant="primary" size="sm" onPress={submit} isDisabled={pending}>
                {pending ? "Saving…" : article.marked ? "Update" : "Save"}
              </Button>
            </Modal.Footer>
          </Modal.Dialog>
        </Modal.Container>
      </Modal.Backdrop>
    </Modal>
  );
}
