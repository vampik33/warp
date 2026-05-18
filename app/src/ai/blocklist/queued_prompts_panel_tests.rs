//! Unit tests for [`super::QueuedPromptsPanelView`].
//!
//! Covers `PRODUCT.md` invariants (12)–(15) (header copy).
//! Live drag-to-reorder is driven by `Workspace::on_tab_drag`-style position lookups instead of
//! a precomputed drop-index translation, so there is no longer a pure helper to unit-test there;
//! the live-reorder behavior is verified end-to-end via the model's `reorder` swap semantics in
//! `queued_query_tests.rs`.
use super::header_label_text;

#[test]
fn header_label_text_renders_count_followed_by_queued() {
    // The Figma design (node 6736:27438) renders `"<N> queued"` regardless of count,
    // not a singular/plural variant of `"queued prompt(s)"`.
    assert_eq!(header_label_text(0), "0 queued");
    assert_eq!(header_label_text(1), "1 queued");
    assert_eq!(header_label_text(2), "2 queued");
    assert_eq!(header_label_text(7), "7 queued");
}
