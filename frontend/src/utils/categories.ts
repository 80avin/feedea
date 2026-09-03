import type { CategoryNode } from "../api/types";

export interface CategoryOption {
  id: string;
  name: string;
  depth: number;
}

export function flattenCategories(nodes: CategoryNode[], depth = 0): CategoryOption[] {
  const result: CategoryOption[] = [];
  for (const node of nodes) {
    result.push({ id: node.category_id, name: node.name, depth });
    result.push(...flattenCategories(node.children, depth + 1));
  }
  return result;
}

interface TreeNode {
  name: string;
  parent_id: string;
}

export interface CategorySegment {
  id: string;
  name: string;
}

/// Path from the root-most ancestor down to `leafId`, each segment carrying its
/// category id so consumers can link to any ancestor (e.g. filter a parent
/// category that only has sub-categories). Empty when `leafId` is unknown.
export function categoryPathFromLeaf(
  nodes: CategoryNode[],
  leafId: string,
): CategorySegment[] {
  const byId = new Map<string, TreeNode>();
  const walk = (list: CategoryNode[]) => {
    for (const node of list) {
      byId.set(node.category_id, { name: node.name, parent_id: node.parent_id });
      walk(node.children);
    }
  };
  walk(nodes);

  const segments: CategorySegment[] = [];
  const seen = new Set<string>();
  let cur: string | undefined = leafId;
  while (cur !== undefined && byId.has(cur) && !seen.has(cur)) {
    seen.add(cur);
    const entry = byId.get(cur);
    if (entry === undefined) {
      break;
    }
    segments.unshift({ id: cur, name: entry.name });
    cur = entry.parent_id;
  }
  return segments;
}
