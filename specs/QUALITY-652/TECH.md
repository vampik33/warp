# Orchestration Config Header Scrolls With Plan Content
Linear: [QUALITY-652](https://linear.app/warpdotdev/issue/QUALITY-652/orchestration-config-header-should-scroll-away-with-content)
Commit researched: `96e4f09a70368235b9e2f185dff14a663d20f181`
## Context
The orchestration config block is part of the AI document plan UI. The issue is that this block should behave like plan content: when a user scrolls a long plan, the config chrome should scroll away before the plan body continues scrolling.
Relevant current code:
- [`app/src/ai/ai_document_view.rs:285 @ 96e4f09`](https://github.com/warpdotdev/warp/blob/96e4f09a70368235b9e2f185dff14a663d20f181/app/src/ai/ai_document_view.rs#L285-L319) reacts to `OrchestrationConfigUpdated`, lazily creates the config block view, and now refreshes the editor scroll header.
- [`app/src/ai/ai_document_view.rs:439 @ 96e4f09`](https://github.com/warpdotdev/warp/blob/96e4f09a70368235b9e2f185dff14a663d20f181/app/src/ai/ai_document_view.rs#L439-L480) creates the initial `OrchestrationConfigBlockView` when opening an AI document with an existing config.
- [`app/src/ai/ai_document_view.rs:541 @ 96e4f09`](https://github.com/warpdotdev/warp/blob/96e4f09a70368235b9e2f185dff14a663d20f181/app/src/ai/ai_document_view.rs#L541-L558) adapts the AI document config block into a rich-text editor scroll-header renderer.
- [`app/src/notebooks/editor/view.rs:92 @ 96e4f09`](https://github.com/warpdotdev/warp/blob/96e4f09a70368235b9e2f185dff14a663d20f181/app/src/notebooks/editor/view.rs#L92-L221) introduces the reusable wrapper element that combines optional header chrome with rich-text content.
- [`app/src/notebooks/editor/view.rs:1601 @ 96e4f09`](https://github.com/warpdotdev/warp/blob/96e4f09a70368235b9e2f185dff14a663d20f181/app/src/notebooks/editor/view.rs#L1601-L1681) stores header renderer, measured height, and consumed scroll offset on `RichTextEditorView`.
## Proposed changes
Keep the orchestration config block owned by `AIDocumentView`, but render it inside the embedded `RichTextEditorView` as an optional scroll header instead of as a sibling above the editor.
The editor exposes `ScrollHeaderRenderer`, a minimal callback that returns optional header chrome for the current render pass. `AIDocumentView::update_editor_scroll_header` supplies a renderer when an orchestration config block exists and clears it otherwise.
`RichTextWithScrollHeaderElement` wraps the existing `RichTextElement` and implements `Element` plus `ScrollableElement`:
- During layout, measure the header, reserve only its visible height, and lay out rich text in the remaining space.
- During paint, draw rich text below the visible header and draw the header shifted upward by the consumed header scroll amount.
- For scrollbar data, report combined header-plus-content size so the scrollable range matches what the user sees.
- For wheel input, dispatch `EditorViewAction::ScrollWithHeader` so scroll behavior can coordinate header and editor content state.
`RichTextEditorView::scroll_with_header` owns the scroll ordering:
- Scrolling down consumes the header first; once the header is fully hidden, remaining delta scrolls the editor content.
- Scrolling up scrolls editor content back toward the top first; the header reappears only after content scroll is exhausted.
- If content scroll changes through another path, `reconcile_scroll_header_after_content_scroll` treats any non-zero content scroll as meaning the header is fully consumed.
The existing AI document render tree remains otherwise unchanged: `AIDocumentView::render` now renders only the editor container in the pane body, while the config block is injected through the editor header path.
## Testing and validation
The integration test coverage is the main validation because the behavior depends on layout, scroll wheel events, and render-state offsets:
- [`app/src/integration_testing/ai_document.rs:40 @ 96e4f09`](https://github.com/warpdotdev/warp/blob/96e4f09a70368235b9e2f185dff14a663d20f181/app/src/integration_testing/ai_document.rs#L40-L204) adds helpers to create an AI document, attach an approved orchestration config, send precise scroll events, and assert header/content offsets.
- [`crates/integration/src/test/ai_document.rs:29 @ 96e4f09`](https://github.com/warpdotdev/warp/blob/96e4f09a70368235b9e2f185dff14a663d20f181/crates/integration/src/test/ai_document.rs#L29-L72) verifies the full sequence: no header before config, header starts at top, header partially hides before content scrolls, content scrolls after the header is hidden, content scrolls back before the header reappears, and both return to top.
- [`crates/integration/src/test.rs:6 @ 96e4f09`](https://github.com/warpdotdev/warp/blob/96e4f09a70368235b9e2f185dff14a663d20f181/crates/integration/src/test.rs#L6-L45), [`crates/integration/src/bin/integration.rs:377 @ 96e4f09`](https://github.com/warpdotdev/warp/blob/96e4f09a70368235b9e2f185dff14a663d20f181/crates/integration/src/bin/integration.rs#L377), and [`crates/integration/tests/integration/ui_tests.rs:238 @ 96e4f09`](https://github.com/warpdotdev/warp/blob/96e4f09a70368235b9e2f185dff14a663d20f181/crates/integration/tests/integration/ui_tests.rs#L238) register the test in the integration harness.
Implementation validation should run `cargo fmt` and the targeted integration test. Full `nextest`/presubmit is not required for this focused branch unless preparing the PR.
## Parallelization
No sub-agents are proposed. The change is localized to a shared editor wrapper, one AI document call site, and one integration test, so parallel implementation would add coordination cost around the same scroll state rather than reduce wall-clock time.
