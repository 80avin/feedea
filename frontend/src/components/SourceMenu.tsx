import { useEffect, useState } from "react";
import { Button, Dropdown, Input, Modal, useOverlayState } from "@heroui/react";
import {
  ArrowPathIcon,
  ArrowTopRightOnSquareIcon,
  CheckCircleIcon,
  EllipsisVerticalIcon,
  PencilIcon,
  TrashIcon,
} from "@heroicons/react/24/outline";
import type { FeedSummary } from "../api/types";
import { useCategories, useDeleteFeed, useFeedRead, useRefreshFeed, useUpdateFeed } from "../state/hooks";
import { flattenCategories } from "../utils/categories";
import { formatError } from "./Feedback";

function RenameDialog({ feed, open, onClose }: { feed: FeedSummary; open: boolean; onClose: () => void }) {
  const state = useOverlayState({
    isOpen: open,
    onOpenChange: (isOpen) => {
      if (!isOpen) onClose();
    },
  });
  const updateFeed = useUpdateFeed();
  const { data: categoriesData } = useCategories();
  const categories = flattenCategories(categoriesData?.categories ?? []);
  const [title, setTitle] = useState(feed.title);
  const [categoryId, setCategoryId] = useState(feed.category_id);
  const [error, setError] = useState("");

  useEffect(() => {
    if (open) {
      setTitle(feed.title);
      setCategoryId(feed.category_id);
      setError("");
    }
  }, [open, feed]);

  const submit = async () => {
    setError("");
    try {
      await updateFeed.mutateAsync({
        id: feed.id,
        title: title.trim() || undefined,
        category_id: categoryId || undefined,
      });
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
              <Modal.Heading>Edit source</Modal.Heading>
            </Modal.Header>
            <Modal.Body>
              <label className="flex flex-col gap-1.5">
                <span className="text-sm font-medium text-app-text-2">Title</span>
                <Input name="title" value={title} onChange={(e) => setTitle(e.target.value)} aria-label="Title" />
              </label>
              <label className="flex flex-col gap-1.5">
                <span className="text-sm font-medium text-app-text-2">Category</span>
                <select
                  value={categoryId}
                  onChange={(e) => setCategoryId(e.target.value)}
                  className="rounded-md border border-app-border-strong bg-app-surface px-3 py-2 text-sm text-app-text outline-none focus:border-accent"
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
              {error && <p className="text-sm text-red-600 dark:text-red-400">{error}</p>}
            </Modal.Body>
            <Modal.Footer>
              <Button variant="ghost" size="sm" onPress={onClose} isDisabled={updateFeed.isPending}>
                Cancel
              </Button>
              <Button variant="primary" size="sm" onPress={submit} isDisabled={updateFeed.isPending}>
                {updateFeed.isPending ? "Saving…" : "Save"}
              </Button>
            </Modal.Footer>
          </Modal.Dialog>
        </Modal.Container>
      </Modal.Backdrop>
    </Modal>
  );
}

function DeleteDialog({ feed, open, onClose }: { feed: FeedSummary; open: boolean; onClose: () => void }) {
  const state = useOverlayState({
    isOpen: open,
    onOpenChange: (isOpen) => {
      if (!isOpen) onClose();
    },
  });
  const deleteFeed = useDeleteFeed();
  const [error, setError] = useState("");

  const confirm = async () => {
    setError("");
    try {
      await deleteFeed.mutateAsync({ id: feed.id });
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
              <Modal.Heading>Delete source</Modal.Heading>
            </Modal.Header>
            <Modal.Body>
              <p className="text-sm text-app-text-2">
                Delete “{feed.title}” and remove its articles? This cannot be undone.
              </p>
              {error && <p className="text-sm text-red-600 dark:text-red-400">{error}</p>}
            </Modal.Body>
            <Modal.Footer>
              <Button variant="ghost" size="sm" onPress={onClose} isDisabled={deleteFeed.isPending}>
                Cancel
              </Button>
              <Button variant="danger" size="sm" onPress={confirm} isDisabled={deleteFeed.isPending}>
                {deleteFeed.isPending ? "Deleting…" : "Delete"}
              </Button>
            </Modal.Footer>
          </Modal.Dialog>
        </Modal.Container>
      </Modal.Backdrop>
    </Modal>
  );
}

export default function SourceMenu({ feed }: { feed: FeedSummary }) {
  const [editOpen, setEditOpen] = useState(false);
  const [deleteOpen, setDeleteOpen] = useState(false);
  const refresh = useRefreshFeed();
  const markRead = useFeedRead();
  const website = feed.website ?? feed.feed_url;

  const openSite = () => {
    if (website) {
      window.open(website, "_blank", "noopener");
    }
  };

  return (
    <>
      <Dropdown>
        <Dropdown.Trigger
          aria-label={`Menu for ${feed.title}`}
          className="rounded-md p-1 text-app-text-muted hover:bg-app-hover hover:text-app-text"
        >
          <EllipsisVerticalIcon className="h-5 w-5" />
        </Dropdown.Trigger>
        <Dropdown.Popover>
          <Dropdown.Menu>
            <Dropdown.Item isDisabled={!website} onAction={openSite}>
              <ArrowTopRightOnSquareIcon className="h-4 w-4" />
              Open
            </Dropdown.Item>
            <Dropdown.Item onAction={() => setEditOpen(true)}>
              <PencilIcon className="h-4 w-4" />
              Edit
            </Dropdown.Item>
            <Dropdown.Item onAction={() => refresh.mutate({ id: feed.id })}>
              <ArrowPathIcon className="h-4 w-4" />
              Refresh
            </Dropdown.Item>
            <Dropdown.Item onAction={() => markRead.mutate({ id: feed.id })}>
              <CheckCircleIcon className="h-4 w-4" />
              Mark all read
            </Dropdown.Item>
            <Dropdown.Item variant="danger" onAction={() => setDeleteOpen(true)}>
              <TrashIcon className="h-4 w-4" />
              Delete
            </Dropdown.Item>
          </Dropdown.Menu>
        </Dropdown.Popover>
      </Dropdown>
      <RenameDialog feed={feed} open={editOpen} onClose={() => setEditOpen(false)} />
      <DeleteDialog feed={feed} open={deleteOpen} onClose={() => setDeleteOpen(false)} />
    </>
  );
}
