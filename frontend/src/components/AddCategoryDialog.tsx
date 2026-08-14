import { useEffect, useState } from "react";
import { Button, Input, Modal, useOverlayState } from "@heroui/react";
import { useAddCategory, useCategories } from "../state/hooks";
import { flattenCategories } from "../utils/categories";
import { formatError } from "./Feedback";

interface AddCategoryDialogProps {
  open: boolean;
  onClose: () => void;
}

export default function AddCategoryDialog({ open, onClose }: AddCategoryDialogProps) {
  const state = useOverlayState({
    isOpen: open,
    onOpenChange: (isOpen) => {
      if (!isOpen) onClose();
    },
  });
  const [name, setName] = useState("");
  const [parentId, setParentId] = useState("");
  const [error, setError] = useState("");
  const addCategory = useAddCategory();
  const { data: categoriesData } = useCategories();
  const categories = flattenCategories(categoriesData?.categories ?? []);

  useEffect(() => {
    if (open) {
      setName("");
      setParentId("");
      setError("");
    }
  }, [open]);

  const submit = async () => {
    setError("");
    const trimmed = name.trim();
    if (!trimmed) {
      setError("Enter a category name.");
      return;
    }
    try {
      await addCategory.mutateAsync({ name: trimmed, parent_id: parentId || undefined });
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
              <Modal.Heading>Add category</Modal.Heading>
            </Modal.Header>
            <Modal.Body>
              <label className="flex flex-col gap-1.5">
                <span className="text-sm font-medium text-app-text-2">Name</span>
                <Input
                  name="name"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder="e.g. Tech"
                  aria-label="Name"
                />
              </label>
              <label className="flex flex-col gap-1.5">
                <span className="text-sm font-medium text-app-text-2">Parent category</span>
                <select
                  value={parentId}
                  onChange={(e) => setParentId(e.target.value)}
                  className="rounded-md border border-app-border-strong bg-app-surface px-3 py-2 text-sm text-app-text outline-none focus:border-accent"
                >
                  <option value="">None (top level)</option>
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
              <Button variant="ghost" size="sm" onPress={onClose} isDisabled={addCategory.isPending}>
                Cancel
              </Button>
              <Button variant="primary" size="sm" onPress={submit} isDisabled={addCategory.isPending}>
                {addCategory.isPending ? "Adding…" : "Add"}
              </Button>
            </Modal.Footer>
          </Modal.Dialog>
        </Modal.Container>
      </Modal.Backdrop>
    </Modal>
  );
}
