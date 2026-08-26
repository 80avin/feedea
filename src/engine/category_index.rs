use std::collections::{HashMap, HashSet};

use crate::dto::FeedSummary;

pub const TOPLEVEL: &str = "NewsFlash.Toplevel";

pub struct IndexInputs {
    pub categories: Vec<(String, String)>,
    pub category_mappings: Vec<(String, String)>,
    pub feed_mappings: Vec<(String, String)>,
    pub feed_titles: Vec<(String, String)>,
    pub unread_by_feed: HashMap<String, i64>,
    pub path_by_id: HashMap<String, Vec<String>>,
    pub feed_summaries: Vec<FeedSummary>,
}

pub struct CategoryIndex {
    pub category_order: Vec<String>,
    pub children_by_parent: HashMap<String, Vec<String>>,
    pub name_by_id: HashMap<String, String>,
    pub path_by_id: HashMap<String, Vec<String>>,
    pub direct_feed_cats: HashSet<String>,
    pub cat_by_feed: HashMap<String, String>,
    pub title_by_feed: HashMap<String, String>,
    pub unread_direct: HashMap<String, i64>,
    pub unread_rolled_up: HashMap<String, i64>,
    pub feed_summaries: Vec<FeedSummary>,
}

impl CategoryIndex {
    pub fn build(inputs: IndexInputs) -> CategoryIndex {
        let IndexInputs {
            categories,
            category_mappings,
            feed_mappings,
            feed_titles,
            unread_by_feed,
            path_by_id,
            feed_summaries,
        } = inputs;

        let mut category_order = Vec::new();
        let mut name_by_id: HashMap<String, String> = HashMap::new();
        for (id, label) in &categories {
            category_order.push(id.clone());
            name_by_id.insert(id.clone(), label.clone());
        }

        let mut children_by_parent: HashMap<String, Vec<String>> = HashMap::new();
        for (parent, child) in &category_mappings {
            children_by_parent
                .entry(parent.clone())
                .or_default()
                .push(child.clone());
        }

        let mut direct_feed_cats: HashSet<String> = HashSet::new();
        let mut sorted: Vec<&(String, String)> = feed_mappings.iter().collect();
        sorted.sort_by(|a, b| {
            let ak = (a.1 == TOPLEVEL, a.0.as_str(), a.1.as_str());
            let bk = (b.1 == TOPLEVEL, b.0.as_str(), b.1.as_str());
            ak.cmp(&bk)
        });
        let mut cat_by_feed: HashMap<String, String> = HashMap::new();
        for (feed_id, category_id) in sorted {
            direct_feed_cats.insert(category_id.clone());
            cat_by_feed
                .entry(feed_id.clone())
                .or_insert_with(|| category_id.clone());
        }

        let title_by_feed: HashMap<String, String> = feed_titles.into_iter().collect();

        let mut unread_direct: HashMap<String, i64> = HashMap::new();
        for (feed_id, category_id) in &feed_mappings {
            let n = unread_by_feed.get(feed_id).copied().unwrap_or(0);
            *unread_direct.entry(category_id.clone()).or_insert(0) += n;
        }

        let mut unread_rolled_up: HashMap<String, i64> = HashMap::new();
        let mut all_cats: HashSet<String> = unread_direct.keys().cloned().collect();
        all_cats.extend(category_order.iter().cloned());
        all_cats.insert(TOPLEVEL.to_string());
        for cat in all_cats {
            let sum = crate::engine::queries::descendant_category_ids(&cat, &children_by_parent)
                .iter()
                .filter_map(|id| unread_direct.get(id))
                .sum();
            unread_rolled_up.insert(cat, sum);
        }

        CategoryIndex {
            category_order,
            children_by_parent,
            name_by_id,
            path_by_id,
            direct_feed_cats,
            cat_by_feed,
            title_by_feed,
            unread_direct,
            unread_rolled_up,
            feed_summaries,
        }
    }

    pub fn is_known(&self, id: &str) -> bool {
        self.category_order.iter().any(|c| c == id)
    }

    pub fn is_direct(&self, id: &str) -> bool {
        self.direct_feed_cats.contains(id)
    }

    /// Card ids in display order: categories-table order filtered to the direct
    /// set, then any remaining direct ids (typically NewsFlash.Toplevel) sorted.
    pub fn card_ids(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .category_order
            .iter()
            .filter(|id| self.direct_feed_cats.contains(*id))
            .cloned()
            .collect();
        let mut extras: Vec<&String> = self
            .direct_feed_cats
            .iter()
            .filter(|id| !self.category_order.contains(id))
            .collect();
        extras.sort();
        out.extend(extras.into_iter().cloned());
        out
    }

    pub fn display_name(&self, id: &str) -> String {
        self.path_by_id
            .get(id)
            .filter(|p| !p.is_empty())
            .map(|p| p.join(" / "))
            .or_else(|| self.name_by_id.get(id).cloned())
            .unwrap_or_else(|| id.to_string())
    }

    pub fn descendants(&self, id: &str) -> Vec<String> {
        crate::engine::queries::descendant_category_ids(id, &self.children_by_parent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs() -> IndexInputs {
        IndexInputs {
            categories: vec![
                ("root".into(), "Root".into()),
                ("leaf".into(), "Leaf".into()),
            ],
            category_mappings: vec![
                (TOPLEVEL.to_string(), "root".into()),
                ("root".into(), "leaf".into()),
            ],
            feed_mappings: vec![
                ("f1".into(), "leaf".into()),
                ("f2".into(), TOPLEVEL.to_string()),
                ("f2".into(), "leaf".into()),
            ],
            feed_titles: vec![("f1".into(), "Feed One".into())],
            unread_by_feed: [("f1".into(), 3i64), ("f2".into(), 5i64)].into_iter().collect(),
            path_by_id: [("root".into(), vec!["Root".to_string()])].into_iter().collect(),
            feed_summaries: vec![],
        }
    }

    #[test]
    fn direct_set_and_cat_by_feed_prefer_real_category_over_toplevel() {
        let idx = CategoryIndex::build(inputs());
        assert!(idx.is_direct("leaf"));
        assert!(idx.is_direct(TOPLEVEL));
        assert!(!idx.is_direct("root"), "container with no direct feed");
        assert_eq!(idx.cat_by_feed.get("f2").map(String::as_str), Some("leaf"));
        assert_eq!(idx.cat_by_feed.get("f1").map(String::as_str), Some("leaf"));
    }

    #[test]
    fn card_ids_preserve_order_and_exclude_containers() {
        let idx = CategoryIndex::build(inputs());
        assert_eq!(idx.card_ids(), vec!["leaf".to_string(), TOPLEVEL.to_string()]);
    }

    #[test]
    fn unread_direct_is_direct_only_and_rolled_up_includes_descendants() {
        let idx = CategoryIndex::build(inputs());
        // f2 has both a toplevel and a leaf mapping; both rows sum into direct.
        assert_eq!(idx.unread_direct.get("leaf").copied().unwrap_or(0), 8);
        assert_eq!(idx.unread_direct.get(TOPLEVEL).copied().unwrap_or(0), 5);
        // descendants(TOPLEVEL) = [TOPLEVEL, root, leaf] => 5 + 0 + 8
        assert_eq!(idx.unread_rolled_up.get(TOPLEVEL).copied().unwrap_or(0), 13);
        // descendants(root) = [root, leaf] => 0 + 8
        assert_eq!(idx.unread_rolled_up.get("root").copied().unwrap_or(0), 8);
    }

    #[test]
    fn is_known_and_descendants() {
        let idx = CategoryIndex::build(inputs());
        assert!(idx.is_known("root"));
        assert!(!idx.is_known("nope"));
        assert!(idx.descendants(TOPLEVEL).contains(&"root".to_string()));
    }

    #[test]
    fn display_name_prefers_path_then_label_then_raw_id() {
        let idx = CategoryIndex::build(inputs());
        assert_eq!(idx.display_name("root"), "Root");
        assert_eq!(idx.display_name("missing"), "missing");
        assert_eq!(idx.display_name(TOPLEVEL), TOPLEVEL);
    }
}
