use std::collections::{HashMap, HashSet};

use opml::{OPML, Outline};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct OpmlEntry {
    pub index: usize,
    pub title: String,
    pub url: String,
    pub category: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExistingFeed {
    pub id: String,
    pub title: String,
    pub url: Option<String>,
    pub website: Option<String>,
    pub category: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConflictKind {
    UrlIdentical,
    UrlVariant,
    IntraFile,
}

#[derive(Debug, Clone, Serialize)]
pub struct Conflict {
    pub key: usize,
    pub kind: ConflictKind,
    pub opml: OpmlEntry,
    pub matches: Vec<ExistingFeed>,
}

#[derive(Debug, Clone)]
pub struct Classification {
    pub conflicts: Vec<Conflict>,
    pub new_count: usize,
    pub exact_duplicates: usize,
    pub skipped: HashSet<usize>,
}

fn collect(outlines: &[Outline], category: &str, index: &mut usize, out: &mut Vec<OpmlEntry>) {
    for outline in outlines {
        if let Some(xml_url) = &outline.xml_url {
            let title = outline
                .title
                .clone()
                .filter(|t| !t.is_empty())
                .or_else(|| Some(outline.text.clone()))
                .unwrap_or_else(|| "No Title".to_string());
            out.push(OpmlEntry {
                index: *index,
                title,
                url: xml_url.clone(),
                category: category.to_string(),
            });
            *index += 1;
        } else {
            let title = outline
                .title
                .clone()
                .filter(|t| !t.is_empty())
                .or_else(|| Some(outline.text.clone()))
                .unwrap_or_default();
            let child_category = if title.is_empty() {
                category.to_string()
            } else {
                title
            };
            collect(&outline.outlines, &child_category, index, out);
        }
    }
}

pub fn parse_entries(opml_str: &str) -> anyhow::Result<Vec<OpmlEntry>> {
    let doc = OPML::from_str(opml_str).map_err(|e| anyhow::anyhow!("invalid opml: {e}"))?;
    let mut entries = Vec::new();
    let mut index = 0;
    collect(&doc.body.outlines, "", &mut index, &mut entries);
    Ok(entries)
}

pub fn normalize_url(raw: &str) -> Option<String> {
    let parsed = url::Url::parse(raw).ok()?;
    let scheme = parsed.scheme().to_lowercase();
    let host = parsed.host_str()?.to_lowercase();
    let mut normalized = format!("{scheme}://{host}");
    if let Some(port) = parsed.port() {
        normalized.push_str(&format!(":{port}"));
    }
    let path = parsed.path().trim_end_matches('/');
    if !path.is_empty() {
        normalized.push_str(path);
    }
    if let Some(query) = parsed.query() {
        normalized.push('?');
        normalized.push_str(query);
    }
    Some(normalized)
}

pub fn classify(entries: &[OpmlEntry], existing: &[ExistingFeed]) -> Classification {
    let mut conflicts = Vec::new();
    let mut skipped = HashSet::new();
    let mut new_count = 0;
    let mut first_by_url: HashMap<String, usize> = HashMap::new();

    for entry in entries {
        let norm = normalize_url(&entry.url);
        let is_first = norm
            .as_ref()
            .map(|n| !first_by_url.contains_key(n))
            .unwrap_or(true);

        let matches: Vec<ExistingFeed> = existing
            .iter()
            .filter(|f| {
                // never let normalize_url == None on the left match a None url/website:
                // an unparseable entry url must not match website-less existing feeds
                f.id == entry.url
                    || norm.as_ref().is_some_and(|n| {
                        f.url.as_deref().and_then(normalize_url).as_ref() == Some(n)
                            || f.website.as_deref().and_then(normalize_url).as_ref() == Some(n)
                    })
            })
            .cloned()
            .collect();

        let id_match = matches.iter().find(|f| f.id == entry.url);

        if let Some(idm) = id_match {
            if idm.title == entry.title && idm.category == entry.category {
                skipped.insert(entry.index);
            } else {
                conflicts.push(Conflict {
                    key: entry.index,
                    kind: ConflictKind::UrlIdentical,
                    opml: entry.clone(),
                    matches: matches.clone(),
                });
            }
        } else if !matches.is_empty() {
            conflicts.push(Conflict {
                key: entry.index,
                kind: ConflictKind::UrlVariant,
                opml: entry.clone(),
                matches: matches.clone(),
            });
        } else if !is_first {
            let n = norm.as_ref().unwrap();
            let first_index = first_by_url[n];
            let first_entry = entries.iter().find(|e| e.index == first_index).unwrap();
            let synthetic = ExistingFeed {
                id: format!("__file__:{first_index}"),
                title: first_entry.title.clone(),
                url: Some(first_entry.url.clone()),
                website: None,
                category: first_entry.category.clone(),
            };
            if synthetic.title == entry.title && synthetic.category == entry.category {
                skipped.insert(entry.index);
            } else {
                conflicts.push(Conflict {
                    key: entry.index,
                    kind: ConflictKind::IntraFile,
                    opml: entry.clone(),
                    matches: vec![synthetic],
                });
            }
        } else {
            new_count += 1;
        }

        if let Some(n) = norm {
            first_by_url.entry(n).or_insert(entry.index);
        }
    }

    Classification {
        conflicts,
        new_count,
        exact_duplicates: skipped.len(),
        skipped,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OPML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <body>
    <outline text="Tech" title="Tech">
      <outline text="Feed A" title="Feed A" type="rss" xmlUrl="https://example.com/feed.xml"/>
      <outline text="Feed A Again" title="Feed A Again" type="rss" xmlUrl="https://example.com/feed.xml/"/>
    </outline>
    <outline text="Feed B" title="Feed B" type="rss" xmlUrl="http://example.org/b/"/>
  </body>
</opml>"#;

    fn existing() -> Vec<ExistingFeed> {
        vec![
            ExistingFeed {
                id: "https://example.com/feed.xml".to_string(),
                title: "Feed A".to_string(),
                url: Some("https://example.com/feed.xml".to_string()),
                website: Some("https://example.com".to_string()),
                category: "Tech".to_string(),
            },
            ExistingFeed {
                id: "http://example.org/b".to_string(),
                title: "Feed B".to_string(),
                url: Some("http://example.org/b".to_string()),
                website: None,
                category: "".to_string(),
            },
        ]
    }

    #[test]
    fn parses_feeds_with_category_path_and_indices() {
        let entries = parse_entries(OPML).unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].index, 0);
        assert_eq!(entries[0].title, "Feed A");
        assert_eq!(entries[0].url, "https://example.com/feed.xml");
        assert_eq!(entries[0].category, "Tech");
        assert_eq!(entries[1].index, 1);
        assert_eq!(entries[1].category, "Tech");
        assert_eq!(entries[2].index, 2);
        assert_eq!(entries[2].category, "");
    }

    #[test]
    fn normalize_url_strips_trailing_slash_and_fragment() {
        assert_eq!(
            normalize_url("HTTP://Example.COM:443/feed.xml/"),
            normalize_url("http://example.com:443/feed.xml#frag")
        );
        assert_eq!(
            normalize_url("https://example.com"),
            Some("https://example.com".into())
        );
        assert_eq!(normalize_url("not a url"), None);
    }

    #[test]
    fn classifies_url_identical_and_url_variant_conflicts_and_new() {
        // Feed A exists with identical raw url -> url-identical conflict (title matches, so nothing)
        // Use a variant title to force a conflict.
        let mut existing = existing();
        existing[0].title = "Renamed".to_string();
        let entries = parse_entries(OPML).unwrap();
        let c = classify(&entries, &existing);
        assert_eq!(c.new_count, 0);
        assert_eq!(c.exact_duplicates, 0);
        // entry 0: url-identical (id == url, title differs)
        // entry 1: url-variant (trailing-slash variant of Feed A's url)
        // entry 2: url-variant (trailing-slash variant of Feed B's url)
        let kinds: Vec<ConflictKind> = c.conflicts.iter().map(|x| x.kind).collect();
        assert!(kinds.contains(&ConflictKind::UrlIdentical));
        assert!(kinds.contains(&ConflictKind::UrlVariant));
        let b = c.conflicts.iter().find(|x| x.key == 2).unwrap();
        assert_eq!(b.matches[0].id, "http://example.org/b");
        assert_eq!(b.kind, ConflictKind::UrlVariant);
    }

    #[test]
    fn classifies_intra_file_conflict_when_title_differs() {
        let opml = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <body>
    <outline text="First" title="First" type="rss" xmlUrl="https://example.com/dup"/>
    <outline text="Second" title="Second" type="rss" xmlUrl="https://example.com/dup/"/>
  </body>
</opml>"#;
        let entries = parse_entries(opml).unwrap();
        let c = classify(&entries, &[]);
        assert_eq!(c.new_count, 1);
        let conflict = c.conflicts.iter().find(|x| x.key == 1).unwrap();
        assert_eq!(conflict.kind, ConflictKind::IntraFile);
        assert_eq!(conflict.matches.len(), 1);
        assert_eq!(conflict.matches[0].id, "__file__:0");
        assert_eq!(conflict.matches[0].title, "First");
        assert_eq!(c.exact_duplicates, 0);
    }

    #[test]
    fn classifies_exact_duplicate_as_skipped() {
        let entries = parse_entries(OPML).unwrap();
        let c = classify(&entries, &existing());
        // entry 0 matches Feed A exactly -> skipped
        assert_eq!(c.exact_duplicates, 1);
        assert!(c.skipped.contains(&0));
        // entry 1: url-variant (trailing-slash variant of Feed A's url, matches Feed A)
        // entry 2: url-variant (trailing-slash variant of Feed B's url, matches Feed B)
        assert_eq!(c.conflicts.len(), 2);
        assert_eq!(c.new_count, 0);
    }

    #[test]
    fn intra_file_exact_duplicate_is_skipped() {
        let opml = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <body>
    <outline text="Same" title="Same" type="rss" xmlUrl="https://example.com/x"/>
    <outline text="Same" title="Same" type="rss" xmlUrl="https://example.com/x"/>
  </body>
</opml>"#;
        let entries = parse_entries(opml).unwrap();
        let c = classify(&entries, &[]);
        assert_eq!(c.exact_duplicates, 1);
        assert!(c.skipped.contains(&1));
        assert_eq!(c.new_count, 1);
    }

    #[test]
    fn unparseable_entry_url_is_new_not_a_conflict() {
        // Feed B in existing() has website: None. A malformed/scheme-less entry url
        // normalizes to None and must NOT match it via the None == None path.
        let opml = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <body>
    <outline text="Bogus" title="Bogus" type="rss" xmlUrl="www.example.com/feed.xml"/>
  </body>
</opml>"#;
        let entries = parse_entries(opml).unwrap();
        let c = classify(&entries, &existing());
        assert_eq!(c.new_count, 1);
        assert_eq!(c.conflicts.len(), 0);
        assert_eq!(c.exact_duplicates, 0);
    }
}
