use percent_encoding::{NON_ALPHANUMERIC, percent_encode};
use scraper::node::{Element, Node};
use scraper::{Html, Selector};
use url::Url;

use html5ever::QualName;

pub fn rewrite_html(html: &str, base_url: &str) -> String {
    let base = match Url::parse(base_url) {
        Ok(b) => b,
        Err(_) => return html.to_string(),
    };
    let mut doc = Html::parse_fragment(html);
    let a_sel = Selector::parse("a").unwrap();
    let img_sel = Selector::parse("img").unwrap();

    let a_updates: Vec<_> = doc
        .select(&a_sel)
        .filter_map(|a| {
            let href = a.value().attr("href")?;
            let abs = base.join(href).ok()?;
            Some((a.id(), abs.as_str().to_string()))
        })
        .collect();
    for (id, abs) in a_updates {
        if let Some(mut node) = doc.tree.get_mut(id)
            && let Node::Element(elem) = node.value()
        {
            set_attr(elem, "href", &abs);
        }
    }

    let img_updates: Vec<_> = doc
        .select(&img_sel)
        .filter_map(|img| {
            let src = img.value().attr("src")?;
            let abs = base.join(src).ok()?;
            if abs.scheme() != "http" && abs.scheme() != "https" {
                return None;
            }
            Some((img.id(), abs.as_str().to_string()))
        })
        .collect();
    for (id, abs) in img_updates {
        if let Some(mut node) = doc.tree.get_mut(id)
            && let Node::Element(elem) = node.value()
        {
            let encoded = percent_encode(abs.as_bytes(), NON_ALPHANUMERIC);
            set_attr(elem, "data-original", &abs);
            set_attr(elem, "src", &format!("/img?u={encoded}"));
        }
    }
    doc.html()
}

fn set_attr(elem: &mut Element, name: &str, value: &str) {
    let local = html5ever::LocalName::from(name);
    if let Some((_, v)) = elem.attrs.iter_mut().find(|(qn, _)| qn.local == local) {
        *v = scraper::StrTendril::from(value);
    } else {
        elem.attrs.push((
            QualName::new(None, html5ever::Namespace::from(""), local),
            scraper::StrTendril::from(value),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "https://example.com/articles/some-post";

    #[test]
    fn relative_link_becomes_absolute() {
        let out = rewrite_html(r#"<p><a href="/rel">link</a></p>"#, BASE);
        assert!(
            out.contains(r#"href="https://example.com/rel""#),
            "got: {out}"
        );
    }

    #[test]
    fn absolute_image_is_proxied_and_preserves_original() {
        let out = rewrite_html(r#"<img src="https://x/y.png">"#, BASE);
        let encoded = percent_encode(b"https://x/y.png", NON_ALPHANUMERIC);
        assert!(
            out.contains(&format!("src=\"/img?u={encoded}\"")),
            "got: {out}"
        );
        assert!(
            out.contains(r#"data-original="https://x/y.png""#),
            "got: {out}"
        );
    }

    #[test]
    fn relative_image_src_resolves() {
        let out = rewrite_html(r#"<img src="/img.png">"#, BASE);
        let encoded = percent_encode(b"https://example.com/img.png", NON_ALPHANUMERIC);
        assert!(
            out.contains(&format!("src=\"/img?u={encoded}\"")),
            "got: {out}"
        );
        assert!(
            out.contains(r#"data-original="https://example.com/img.png""#),
            "got: {out}"
        );
    }

    #[test]
    fn data_uri_image_is_not_proxied() {
        let out = rewrite_html(r#"<img src="data:image/png;base64,AAAA">"#, BASE);
        assert!(
            out.contains(r#"src="data:image/png;base64,AAAA""#),
            "got: {out}"
        );
        assert!(!out.contains("/img?u="), "got: {out}");
        assert!(!out.contains("data-original"), "got: {out}");
    }

    #[test]
    fn non_http_scheme_image_is_not_proxied() {
        let out = rewrite_html(r#"<img src="ftp://example.com/x.png">"#, BASE);
        assert!(
            out.contains(r#"src="ftp://example.com/x.png""#),
            "got: {out}"
        );
        assert!(!out.contains("/img?u="), "got: {out}");
        assert!(!out.contains("data-original"), "got: {out}");
    }

    #[test]
    fn external_absolute_link_stays_absolute() {
        let out = rewrite_html(r#"<a href="https://external.org/page">ext</a>"#, BASE);
        assert!(
            out.contains(r#"href="https://external.org/page""#),
            "got: {out}"
        );
    }

    #[test]
    fn invalid_base_returns_input_unchanged() {
        let html = r#"<a href="/rel">link</a>"#;
        assert_eq!(rewrite_html(html, "not a url"), html);
    }
}
