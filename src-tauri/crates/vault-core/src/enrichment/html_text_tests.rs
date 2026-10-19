//! Responsibilities: pin the exclusion of non-content HTML subtrees.
//! Not responsible for: network fetching, site adapters or model prompts.
//! Failure modes: executable/style text survives in body blocks, inside list items, or the
//! no-block fallback; blocks nested in excluded subtrees slip through the CSS selector.
use super::*;

#[test]
fn readable_html_excludes_non_content_subtrees_in_blocks_and_fallback() {
    let noise = "<script>scriptNoise</script><style>styleNoise</style><noscript>noscriptNoise</noscript><template><p>templateNoise</p></template><svg><text>svgNoise</text></svg>";
    for body in [
        format!("<p>Visible words</p>{noise}"),
        format!("<ul><li>Visible{noise} words</li></ul>"),
        format!("Visible{noise} words"),
    ] {
        let html = format!("<html><head><title>Page</title></head><body>{body}</body></html>");
        let result = build_enrichment_result_from_html(
            "https://example.com",
            None,
            "text/html".into(),
            &html,
        );
        assert_eq!(result.readable_text.as_deref(), Some("Visible words"), "{body}");
        assert_eq!(result.snippets, ["Visible words"]);
    }
    let result = build_enrichment_result_from_html(
        "https://example.com",
        None,
        "text/html".into(),
        &format!("<body>{noise}</body>"),
    );
    assert!(result.readable_text.is_none());
}
