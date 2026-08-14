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
