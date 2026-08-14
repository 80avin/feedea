import { useEffect, useState } from "react";
import { Button, Input, Modal, useOverlayState } from "@heroui/react";
import clsx from "clsx";
import type { DiscoverResponse } from "../api/types";
import { useAddSource, useCategories, useDiscover } from "../state/hooks";
import { flattenCategories } from "../utils/categories";
import { formatError } from "./Feedback";

interface AddSourceDialogProps {
  open: boolean;
  onClose: () => void;
}

export default function AddSourceDialog({ open, onClose }: AddSourceDialogProps) {
  const state = useOverlayState({
    isOpen: open,
    onOpenChange: (isOpen) => {
      if (!isOpen) onClose();
    },
  });
  const [url, setUrl] = useState("");
  const [title, setTitle] = useState("");
  const [categoryId, setCategoryId] = useState("");
  const [chosenFeedUrl, setChosenFeedUrl] = useState("");
  const [discovered, setDiscovered] = useState<DiscoverResponse | null>(null);
  const [error, setError] = useState("");
  const discover = useDiscover();
  const addSource = useAddSource();
  const { data: categoriesData } = useCategories();
  const categories = flattenCategories(categoriesData?.categories ?? []);
  const pending = discover.isPending || addSource.isPending;

  useEffect(() => {
    if (open) {
      setUrl("");
      setTitle("");
      setCategoryId("");
      setChosenFeedUrl("");
      setDiscovered(null);
      setError("");
    }
  }, [open]);

  const onUrlChange = (value: string) => {
    setUrl(value);
    setChosenFeedUrl("");
    setTitle("");
  };

  const onFetchTitle = async () => {
    setError("");
    setDiscovered(null);
    setChosenFeedUrl("");
    const trimmed = url.trim();
    if (!trimmed) {
      setError("Enter a URL first.");
      return;
    }
    try {
      const result = await discover.mutateAsync({ url: trimmed });
      setDiscovered(result);
      if (result.title) {
        setTitle(result.title);
      }
      if (result.feed_url) {
        setChosenFeedUrl(result.feed_url);
      }
    } catch (e) {
      setError(formatError(e));
    }
  };

  const submit = async () => {
    setError("");
    const finalUrl = (chosenFeedUrl || url).trim();
    if (!finalUrl) {
      setError("Enter a URL.");
      return;
    }
    try {
      await addSource.mutateAsync({
        url: finalUrl,
        title: title.trim() || undefined,
        category_id: categoryId || undefined,
      });
      onClose();
    } catch (e) {
      setError(formatError(e));
    }
  };

  const nothingFound = discovered !== null && !discovered.feed_url && discovered.alternatives.length === 0;

  return (
    <Modal state={state}>
      <Modal.Backdrop>
        <Modal.Container>
          <Modal.Dialog>
            <Modal.Header>
              <Modal.Heading>Add source</Modal.Heading>
            </Modal.Header>
            <Modal.Body>
              <label className="flex flex-col gap-1.5">
                <span className="text-sm font-medium text-zinc-300">URL</span>
                <div className="flex gap-2">
                  <Input
                    name="url"
                    value={url}
                    onChange={(e) => onUrlChange(e.target.value)}
                    placeholder="https://example.com/feed.xml"
                    aria-label="URL"
                  />
                  <Button variant="secondary" size="sm" onPress={onFetchTitle} isDisabled={pending || !url.trim()}>
                    {discover.isPending ? "Fetching…" : "Fetch title"}
                  </Button>
                </div>
              </label>

              {nothingFound && (
                <p className="text-sm text-amber-400">
                  No feed found at that URL — it will be added as-is.
                </p>
              )}

              {discovered && discovered.alternatives.length > 0 && (
                <div className="flex flex-col gap-1">
                  <span className="text-xs font-medium text-zinc-400">Feeds found:</span>
                  {discovered.alternatives.map((alt) => (
                    <button
                      key={alt.url}
                      type="button"
                      onClick={() => setChosenFeedUrl(alt.url)}
                      className={clsx(
                        "flex flex-col items-start rounded-md border px-3 py-2 text-left text-sm",
                        chosenFeedUrl === alt.url
                          ? "border-zinc-400 bg-zinc-800"
                          : "border-zinc-800 hover:border-zinc-700",
                      )}
                    >
                      <span className="font-medium text-zinc-100">{alt.label || alt.url}</span>
                      <span className="break-all text-xs text-zinc-500">{alt.url}</span>
                    </button>
                  ))}
                </div>
              )}

              <label className="flex flex-col gap-1.5">
                <span className="text-sm font-medium text-zinc-300">Title</span>
                <Input
                  name="title"
                  value={title}
                  onChange={(e) => setTitle(e.target.value)}
                  placeholder="Optional — fetched automatically"
                  aria-label="Title"
                />
              </label>

              <label className="flex flex-col gap-1.5">
                <span className="text-sm font-medium text-zinc-300">Category</span>
                <select
                  value={categoryId}
                  onChange={(e) => setCategoryId(e.target.value)}
                  className="rounded-md border border-zinc-700 bg-zinc-900 px-3 py-2 text-sm text-zinc-100 outline-none focus:border-zinc-500"
                >
                  <option value="">No category</option>
                  {categories.map((c) => (
                    <option key={c.id} value={c.id}>
                      {"\u00A0".repeat(c.depth * 2)}
                      {c.name}
                    </option>
                  ))}
                </select>
              </label>

              {error && <p className="text-sm text-red-400">{error}</p>}
            </Modal.Body>
            <Modal.Footer>
              <Button variant="ghost" size="sm" onPress={onClose} isDisabled={pending}>
                Cancel
              </Button>
              <Button variant="primary" size="sm" onPress={submit} isDisabled={pending || !url.trim()}>
                {addSource.isPending ? "Adding…" : "Add"}
              </Button>
            </Modal.Footer>
          </Modal.Dialog>
        </Modal.Container>
      </Modal.Backdrop>
    </Modal>
  );
}
