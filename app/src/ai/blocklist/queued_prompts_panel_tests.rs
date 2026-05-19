//! Unit tests for [`super::QueuedPromptsPanelView`].
//!
//! Live drag-to-reorder is driven by `Workspace::on_tab_drag`-style position lookups instead of
//! a precomputed drop-index translation, so there is no longer a pure helper to unit-test there;
//! the live-reorder behavior is verified end-to-end via the model's `reorder` swap semantics in
//! `queued_query_tests.rs`.
use super::header_label_text;

#[test]
fn header_label_text_renders_count_followed_by_queued() {
    assert_eq!(header_label_text(0), "0 queued");
    assert_eq!(header_label_text(1), "1 queued");
    assert_eq!(header_label_text(2), "2 queued");
    assert_eq!(header_label_text(7), "7 queued");
}
