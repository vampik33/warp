# Queued Prompts UI — Technical Spec
See `specs/REMOTE-1543/PRODUCT.md` for the user-visible behavior. This document covers implementation only.
## Context
The queue replaces a single-slot pending-prompt mechanism that previously powered `/queue`, the auto-queue toggle, `/compact-and`, `/fork-and-compact`, and the Cloud Mode "waiting on harness" indicator. The single slot is gone; a per-conversation FIFO model now backs all of those trigger surfaces, and a new collapsible panel renders between the warping indicator and the agent input box.
### Components
- `QueuedQueryModel` in `app/src/ai/blocklist/queued_query.rs` — the per-conversation queue (`HashMap<AIConversationId, Vec<QueuedQuery>>` plus edit / collapse state).
- `QueuedPromptsPanelView` in `app/src/ai/blocklist/queued_prompts_panel.rs` — the rendered panel (header + rows + inline editor + drag-and-drop).
- `BlocklistAIContextModel` in `app/src/ai/blocklist/context_model.rs` — owns `queued_query_model: ModelHandle<QueuedQueryModel>` and forwards relevant history / agent-view-exit events into it.
- `TerminalView` in `app/src/terminal/view.rs` — instantiates the panel, wires it into `Input`, and drains the queue on `FinishedReceivingOutput`.
- `AmbientAgentViewModel` in `app/src/terminal/view/ambient_agent/model.rs` — tracks the cloud-mode-initial-prompt row id so the row can be torn down on harness lifecycle events.
## Implementation
### 1. `QueuedQueryModel` (`app/src/ai/blocklist/queued_query.rs`)
Fields:
- `queues: HashMap<AIConversationId, Vec<QueuedQuery>>` — per-conversation FIFO.
- `editing: Option<EditingRow>` where `EditingRow { conversation_id, query_id }` — at most one row across all conversations may be in edit mode.
- `collapsed: HashSet<AIConversationId>` — per-conversation collapse state (cleared by `clear_for_conversation`).
Types:
- `QueuedQueryId(Uuid)` newtype for stable row addressing across reorder / edit / delete.
- `QueuedQuery { id, text, origin }` with `into_text()` and `text()` accessors.
- `QueuedQueryOrigin` with variants `QueueSlashCommand`, `AutoQueueToggle`, `CompactAnd`, `ForkAndCompact`, `InitialCloudMode`. `is_user_managed()` returns `false` only for `InitialCloudMode`; user-managed rows participate in edit / delete / reorder and auto-fire.
- `AutofireAction { Submit { text } | PopFromEditMode { text } }` — returned by `pop_for_autofire` to tell the caller whether to submit through the normal path or place the text in the input (only when input is empty).
Operations (all emit a `QueuedQueryEvent`):
- `append(conv_id, query, ctx) -> QueuedQueryId`
- `pop_front(conv_id, ctx) -> Option<QueuedQuery>` — used by the Error / Cancelled drain path; clears `editing` if the popped row was being edited.
- `pop_for_autofire(conv_id, edit_text_override: Option<String>, ctx) -> Option<AutofireAction>` — returns `None` when the queue is empty or the head row's origin is non-user-managed (Cloud Mode); pops the head and returns `Submit { text }`, or returns `PopFromEditMode { text }` when the head was the in-edit row (substituting `edit_text_override` when provided).
- `remove_by_id(conv_id, query_id, ctx) -> Option<QueuedQuery>` — also clears `editing` if removing the in-edit row.
- `replace_text_by_id(conv_id, query_id, new_text, ctx)` — no-op for non-user-managed rows or unchanged text.
- `reorder(conv_id, source_id, target_index, ctx)` — no-op for non-user-managed rows; `target_index` is clamped.
- `enter_edit_mode(conv_id, query_id, ctx)` — no-op for non-user-managed rows; implicitly commits any prior edit by emitting `EditCommitted` for the displaced row.
- `commit_edit(new_text, ctx)` — replaces text via `replace_text_by_id` (or deletes the row when `new_text` is empty per `PRODUCT.md` (17)), then clears `editing`.
- `cancel_edit(ctx)` — clears `editing` and emits `EditCancelled`.
- `set_collapsed(conv_id, collapsed, ctx)` — toggles the conversation's collapse set membership.
- `clear_for_conversation(conv_id, ctx)` — drops the queue, collapse state, and edit state for one conversation; emits `Cleared`.
- `clear_all(ctx)` — drops everything (used on agent-view-exit and on `ClearedConversationsInTerminalView`).
- Accessors: `queue_for(conv_id) -> &[QueuedQuery]`, `has_queue(conv_id) -> bool`, `first_text(conv_id) -> Option<&str>`, `editing_row(conv_id) -> Option<QueuedQueryId>`, `is_collapsed(conv_id) -> bool`.
Events: `Appended`, `Removed`, `Replaced`, `Reordered`, `EditEntered`, `EditCommitted`, `EditCancelled`, `CollapseToggled { collapsed }`, `Cleared`.
Per-conversation lifecycle (`PRODUCT.md` (38)–(40)) is enforced by `BlocklistAIContextModel::new` (`context_model.rs:246-289, 303-317`):
- `BlocklistAIHistoryEvent::ClearedConversationsInTerminalView` → `clear_all`.
- `BlocklistAIHistoryEvent::RemoveConversation` / `DeletedConversation` → `clear_for_conversation(conversation_id)`.
- `AgentViewControllerEvent::ExitedAgentView` → `clear_all`.
The model is owned by `BlocklistAIContextModel::queued_query_model: ModelHandle<QueuedQueryModel>` (`context_model.rs:156`), initialized in `BlocklistAIContextModel::new` and `new_for_test`. A `queued_query_model()` accessor exposes it (`context_model.rs:381`).
### 2. `QueuedPromptsPanelView` (`app/src/ai/blocklist/queued_prompts_panel.rs`)
Fields:
- `view_id` (cached for namespacing per-row `SavePosition` ids during live drag).
- `queued_query_model: ModelHandle<QueuedQueryModel>`.
- `ai_context_model: ModelHandle<BlocklistAIContextModel>` — for the active conversation id.
- `edit_editor: ViewHandle<EditorView>` — single shared `EditorView` reused across edit sessions.
- `header_mouse_state`, `row_mouse_states`, `edit_button_mouse_states`, `delete_button_mouse_states: HashMap<QueuedQueryId, MouseStateHandle>`, `row_draggable_states: HashMap<QueuedQueryId, DraggableState>` — created lazily per row and freed when the model emits `Removed`.
- `dragging_query_id: Option<QueuedQueryId>`, `drag_start_index: Option<usize>` — in-flight drag state used to emit reorder telemetry / events on drop.
Actions: `QueuedPromptsPanelAction { ToggleCollapsed, StartEditingRow(id), DeleteRow(id), CommitEdit, CancelEdit, StartDrag(id), DragMoved { rect }, DropEnd }`.
Events (consumed by `TerminalView::handle_queued_prompts_panel_event`, `view.rs:5064-5090`):
- `RowRemoved { query_id, was_via_edit_commit }`
- `RowEdited { query_id }`
- `CollapseToggled { collapsed }`
- `EditCancelled { query_id }`
- `RowEditEntered { query_id }`
- `RowDeletedForInputPlacement { text }` — the panel asks the host to place the deleted row's text in the input editor when it's empty (`PRODUCT.md` (16) / (23)).
- `RowReordered { query_id, from_index, to_index }`.
Visibility gate: `should_render(&self, ctx)` returns `false` unless both `FeatureFlag::QueueSlashCommand` and `FeatureFlag::PendingUserQueryIndicator` are enabled AND the active conversation has at least one queued row (`PRODUCT.md` (2), (4)). The `View::render` impl short-circuits to `Empty::new().finish()` when `should_render` is false.
Render flow:
1. Resolve the active conversation id from `BlocklistAIContextModel::selected_conversation_id`. Bail to `Empty` if none.
2. Render the header row: chevron icon + `"<N> queued"` text, wrapped in `Hoverable` with an `on_click` that dispatches `ToggleCollapsed`.
3. When `!collapsed`, render each row in queue order using a vertical `Flex`. Each row is `[drag handle, prompt text or shared edit editor, hover ? (pencil + trash) : nothing]`, wrapped in `Hoverable`, then `Container` (with optional hover background), then `SavePosition`. User-managed rows are then wrapped in `Draggable::new(state, content).with_drag_axis(DragAxis::VerticalOnly)` with `on_drag_start` → `StartDrag(id)`, `on_drag` → `DragMoved { rect }`, `on_drop` → `DropEnd`. Cloud Mode rows and rows currently in edit mode skip the `Draggable` wrap so their drag handle is inert (`PRODUCT.md` (14), (19), (30)).
4. Live-reorder during drag: `drag_moved` looks up the dragged row's current index, calls `calculate_updated_row_index(panel_view_id, current_index, queue_len, rect, ctx)` (which compares `rect` against each neighbor row's `SavePosition`-cached bounds), and calls `QueuedQueryModel::reorder` whenever the index changes. On `DropEnd`, telemetry / `RowReordered` are emitted only when the row's index actually changed during the drag.
`TerminalView` constructs the panel in `TerminalView::new` (`view.rs:4318-4337`): it creates the panel handle from the context model's `queued_query_model`, stores the panel on `Input::queued_prompts_panel: Option<ViewHandle<QueuedPromptsPanelView>>` (`input.rs:3624`, `set_queued_prompts_panel` at `input.rs:3699`), and subscribes to its events to drive input refocus (`view.rs:4332-4334`).
### 3. Edit-in-place
- `enter_edit_mode(id)` (on the model, dispatched from the row's pencil button) sets the editor contents to the current prompt, sets `editing = Some(EditingRow { conv, id })`, and the panel's `on_focus` re-focuses `edit_editor` if a row is being edited.
- Pressing Enter inside the editor dispatches `CommitEdit`, which reads the editor text and calls `commit_edit` on the model — that calls `replace_text_by_id` (or `remove_by_id` when the new text is empty per `PRODUCT.md` (17)) and clears `editing`. The panel's `RowRemoved { was_via_edit_commit: true }` / `RowEdited` events are emitted from inside the panel.
- Pressing Esc dispatches `CancelEdit`, which calls `cancel_edit` on the model. Focus loss on the panel itself (`View::on_blur` with `is_self_blurred`) commits any in-progress edit as a safety net.
- Clicking the pencil on a different row dispatches `StartEditingRow(other_id)`, which commits the prior edit (the model emits `EditCommitted` for the displaced row) before entering edit mode on the new one (`PRODUCT.md` (20)).
### 4. Drag-to-reorder
- Each user-managed, non-editing row is wrapped in `Draggable::new(draggable_state, row_inner).with_drag_axis(DragAxis::VerticalOnly)`.
- The drag handlers dispatch `QueuedPromptsPanelAction::StartDrag(id) / DragMoved { rect } / DropEnd`. The panel's `drag_moved` does the live reorder by comparing the dragged row's `rect` against neighbor `SavePosition`-cached bounds and calling `QueuedQueryModel::reorder`. This mirrors `Workspace::on_tab_drag` (`workspace/view.rs`).
- Rows in edit mode and Cloud Mode rows skip the `Draggable` wrap entirely, so their drag handle is inert (`PRODUCT.md` (19), (30)). They still register a `SavePosition` so neighbors dragged across them can measure bounds correctly.
### 5. Auto-fire integration
`TerminalView::drain_queued_prompts(conversation_id, finish_reason, ctx)` (`view.rs:5132-5182`) is invoked from `handle_ai_controller_event` on `BlocklistAIControllerEvent::FinishedReceivingOutput` when the existing guards (no active subagent, last AI block finished, etc.) are satisfied:
```rust path=null start=null
match finish_reason {
    FinishReason::Complete => {
        let action = queued_query_model
            .update(ctx, |m, ctx| m.pop_for_autofire(conversation_id, None, ctx));
        match action {
            Some(AutofireAction::Submit { text }) => {
                self.input.update(ctx, |input, ctx| input.submit_queued_prompt(text, ctx));
            }
            Some(AutofireAction::PopFromEditMode { text }) => {
                self.input.update(ctx, |input, ctx| {
                    if input.buffer_text(ctx).is_empty() {
                        input.replace_buffer_content(&text, ctx);
                        input.focus_input_box(ctx);
                    }
                });
            }
            None => {} // empty queue or Cloud Mode head
        }
    }
    FinishReason::Error
    | FinishReason::Cancelled
    | FinishReason::CancelledDuringRequestedCommandExecution => {
        let input_is_empty = self.input.as_ref(ctx).buffer_text(ctx).is_empty();
        if !input_is_empty {
            return; // queue stays intact per PRODUCT.md (35)
        }
        if let Some(query) =
            queued_query_model.update(ctx, |m, ctx| m.pop_front(conversation_id, ctx))
        {
            self.input.update(ctx, |input, ctx| input.replace_buffer_content(query.text(), ctx));
        }
    }
}
```
`pop_for_autofire` semantics:
- Empty queue → `None`.
- Head is non-user-managed (`InitialCloudMode`) → `None`; the harness owns firing (see §8).
- Head is user-managed and in edit mode → pops the row and returns `AutofireAction::PopFromEditMode { text }` (substituting `edit_text_override` if the caller passed one). The caller places `text` in the input when the input is empty per `PRODUCT.md` (21).
- Otherwise → pops the head and returns `AutofireAction::Submit { text }`. The caller routes through `Input::submit_queued_prompt` (`input.rs:13075-13169`), which dispatches the prompt through the normal slash / skill / plain-text detection path.
`drain_queued_prompts` is called from `handle_ai_controller_event` with the active conversation id from the controller event. The current caller passes `None` for `edit_text_override` — the live editor's in-progress text is not threaded through, so the committed row text is what gets placed in the input.
### 6. Trigger surfaces
All five trigger surfaces append directly to `QueuedQueryModel` (no more single-slot callback). The central helper is `TerminalView::enqueue_prompt(prompt, origin, ctx) -> Option<QueuedQueryId>` (`view.rs:5036-5059`), which resolves the active conversation id and calls `QueuedQueryModel::append`.
- **Auto-queue toggle bottleneck** — `Input::maybe_queue_input_for_in_progress_conversation` (`input.rs:13176-13266`) inlines the model append (calling `queued_query_model.update(...).append(conv_id, QueuedQuery::new(prompt, AutoQueueToggle), ctx)` directly) because it runs inside `Input` which already has the active conversation id; it does not go through `enqueue_prompt`. The `/queue` argument unwrap and the buffer clear are unchanged.
- **`/queue` slash command** (`input/slash_commands/mod.rs:1048-1088`) — when the conversation is in progress, appends directly to the model with origin `QueueSlashCommand`. The idle path (`submit_queued_prompt(prompt)`) is unchanged.
- **`WorkspaceAction::QueuePromptForConversation { prompt }`** (`workspace/action.rs:516`, handler at `workspace/view.rs:22444-22460`) — the handler looks up the active session view and calls `terminal.enqueue_prompt(prompt, QueuedQueryOrigin::AutoQueueToggle, ctx)`. The action is kept so cross-pane callers (Cmd-Shift-J keybinding, etc.) keep working.
- **`/compact-and`** (`Workspace::summarize_active_ai_conversation`, `workspace/view.rs:12142-12170`) — dispatches the summarize slash-command request immediately, then calls `terminal.enqueue_prompt(prompt, QueuedQueryOrigin::CompactAnd, ctx)` on the `initial_prompt`. The summarize exchange completes first; the queue's FIFO drain fires the appended row next.
- **`/fork-and-compact`** (`Workspace::handle_forked_conversation_prompts`, `workspace/view.rs:12041-12086`) — same pattern: dispatch the summarize request on the new forked terminal view, then call `terminal.enqueue_prompt(prompt, QueuedQueryOrigin::ForkAndCompact, ctx)` for the initial prompt.
- **Cloud Mode initial prompt** — see §8.
### 7. Removal of the legacy inline pending block
- Deleted `app/src/ai/blocklist/block/pending_user_query_block.rs` and its module export from `app/src/ai/blocklist/block.rs`.
- Deleted `app/src/terminal/view/pending_user_query.rs` and removed its module declaration.
- Removed `RichContentMetadata::PendingUserQuery` and `RichContent::is_pending_user_query` from `app/src/terminal/view/rich_content.rs`.
- Removed the unused `BlockList::unpin_rich_content_from_bottom` and the legacy `pending_user_query_view_id` / `pending_user_query_kind` / `queued_prompt_callback` fields from `TerminalView`.
- `FeatureFlag::PendingUserQueryIndicator` is repurposed to gate the new panel — `QueuedPromptsPanelView::should_render` returns `false` when the flag is off, so prompts continue to queue silently and become visible once the flag flips on.
- Both `FeatureFlag::QueueSlashCommand` and `FeatureFlag::PendingUserQueryIndicator` must be enabled for the panel to render. `QueueSlashCommand` continues to gate the trigger surfaces (`/queue` registration, the auto-queue toggle button, and the slash-command queue routing).
### 8. Cloud Mode initial prompt
The legacy `insert_cloud_mode_queued_user_query_block(prompt, ctx)` callsite is gone. Cloud Mode rows are now queued through the same `enqueue_prompt` helper used for the other trigger surfaces, regardless of harness (Oz or third-party).
- `AmbientAgentViewModel` tracks the row id with a `cloud_mode_queued_query_id: Option<QueuedQueryId>` field (`ambient_agent/model.rs:241`, accessors at `:329-335`). The id is set when the panel row is appended and cleared when the row is removed.
- `TerminalView::handle_ambient_agent_event` (`ambient_agent/view_impl.rs:99-273`) handles cloud-mode lifecycle events:
  - **`DispatchedAgent`** (initial prompt, gated on `CloudModeSetupV2`): rebuilds the display form of the prompt from `request.mode` (so `/plan` / `/orchestrate` prefixes survive the round-trip), calls `self.enqueue_prompt(prompt, QueuedQueryOrigin::InitialCloudMode, ctx)`, and stores the returned id via `set_cloud_mode_queued_query_id`. Skipped for shared-session viewers and transcript viewers (they have no submitted prompt to render).
  - **`FollowupDispatched`** (cloud follow-up while `pending_followup_prompt` is set, gated on `CloudModeSetupV2`): same `enqueue_prompt(..., InitialCloudMode, ctx)` flow.
  - **`HarnessCommandStarted`**, **`NeedsGithubAuth`**, **`Cancelled`**, **`HandoffSnapshotUploadFailed`**, and **`Failed { .. }` when `CloudModeSetupV2` is disabled** all call `self.remove_cloud_mode_queued_query(ctx)` (`view.rs:5094-5122`). That helper looks up the row id on the ambient model, calls `QueuedQueryModel::remove_by_id`, and clears the recorded id.
  - **`Failed { .. }` when `CloudModeSetupV2` is enabled** keeps the queued row in place — the tombstone is inserted below it so the user can see what they had asked for above the failure message.
- `TerminalView` (`view.rs:5611-5620`) also calls `remove_cloud_mode_queued_query` from the `AppendedExchange` handler when `ambient_agent_view_model.is_local_to_cloud_handoff()` is true. For oz local-to-cloud handoff, the first appended exchange is the analog of `HarnessCommandStarted` for non-oz harnesses — that's when the queued-prompt row hands off to the live agent UI.
- `QueuedQueryOrigin::InitialCloudMode` is non-user-managed: `pop_for_autofire` skips it, the panel hides the edit / delete affordances and inert-locks the drag handle (`PRODUCT.md` (14), (30)).
### 9. Telemetry
New events emitted from `QueuedPromptsPanelView` (`server/telemetry/events.rs`):
- `QueuedPromptEdited { origin: TelemetryQueuedQueryOrigin }`
- `QueuedPromptDeleted { origin: TelemetryQueuedQueryOrigin }`
- `QueuedPromptReordered { origin: TelemetryQueuedQueryOrigin, from_index: usize, to_index: usize }`
- `QueuedPromptPanelCollapseToggled { collapsed: bool }`
`TelemetryQueuedQueryOrigin` is the wire-side mirror of `QueuedQueryOrigin` with a `From<QueuedQueryOrigin>` impl so the panel can pass row origins through without exposing the model type. Existing `/queue` and auto-queue events in `TelemetryEvent::SlashCommandAccepted` keep firing.
## End-to-end flow
```mermaid
sequenceDiagram
    participant U as User
    participant I as Input
    participant TV as TerminalView
    participant Q as QueuedQueryModel
    participant P as QueuedPromptsPanelView
    participant AI as BlocklistAIController
    U->>I: submit prompt while agent in-progress (toggle on)
    I->>Q: append(conv_id, QueuedQuery::new(prompt, AutoQueueToggle))
    Q-->>P: QueuedQueryEvent::Appended
    P-->>U: render new row at tail
    AI-->>TV: FinishedReceivingOutput(Complete)
    TV->>Q: pop_for_autofire(conv_id, None)
    Q-->>TV: Some(AutofireAction::Submit { text })
    TV->>I: submit_queued_prompt(text)
    I->>AI: send_queued_user_query_in_conversation
    Note over Q,P: count decrements; next row becomes head
    AI-->>TV: FinishedReceivingOutput(Error|Cancelled)
    alt input is empty
        TV->>Q: pop_front(conv_id)
        Q-->>TV: Some(query)
        TV->>I: replace_buffer_content(query.text)
        Note over Q,P: row removed; auto-fire paused until next Complete
    else input is non-empty
        Note over Q,P: queue untouched; auto-fire paused until next Complete
    end
```
## Tests
Three test modules cover the behavior:
- `app/src/ai/blocklist/queued_query_tests.rs` — `QueuedQueryModel` unit tests covering append, FIFO order, per-conversation isolation, edit lock, commit-replaces-text, commit-empty-deletes, cancel-restores, reorder by id, remove by id, clear (per-conversation and all), Cloud Mode rows reject edit / delete / reorder, and the `pop_for_autofire` matrix (empty queue, Cloud Mode head, edit-mode head with and without `edit_text_override`).
- `app/src/terminal/view/queued_prompts_test.rs` — auto-fire drain logic at the model level, covering `PRODUCT.md` (21), (31)–(37). Constructing a full `TerminalView` in a unit test is impractical, so these tests exercise `QueuedQueryModel::pop_for_autofire` / `pop_front` directly with the same call shapes the host uses.
- `app/src/ai/blocklist/queued_prompts_panel_tests.rs` — `QueuedPromptsPanelView` smoke tests covering rendering when the queue is empty vs. non-empty.
- `app/src/terminal/view_tests.rs` — integration tests for Cloud Mode rows: `cloud_mode_dispatched_agent_inserts_queued_user_query`, `cloud_mode_failed_keeps_queued_prompt_and_hides_input` (validates that `CloudModeSetupV2` keeps the queued row across `Failed` so it appears above the tombstone), and `cloud_mode_followup_dispatched_inserts_queued_user_query`.
Presubmit (`./script/presubmit`) and `cargo nextest run --no-fail-fast --workspace --exclude command-signatures-v2` must pass.
## Risks and mitigations
- **Edit-mode race with auto-fire**: the user can be mid-edit when `Complete` arrives. `pop_for_autofire` atomically pops the row and returns `PopFromEditMode { text }` inside a single `model.update`, so the view never observes a stale "row still rendered as editing" state — it sees a `Removed` event and re-renders. The current caller passes `None` for `edit_text_override`, so the committed row text wins over any uncommitted edits in the live editor.
- **Cloud Mode harness ownership**: `pop_for_autofire` skips `InitialCloudMode` rows. If the head is a Cloud Mode row, auto-fire is a no-op until the harness either fires it or cancels it. Cloud Mode rows are removed by the ambient agent lifecycle handlers (`HarnessCommandStarted`, `NeedsGithubAuth`, `Cancelled`, `HandoffSnapshotUploadFailed`, or the oz `AppendedExchange` handoff path).
- **Two-flag rollout**: the panel only renders when both `QueueSlashCommand` and `PendingUserQueryIndicator` are on. If `QueueSlashCommand` is on alone, prompts queue silently with no UI; rolling the indicator flag concurrently avoids that limbo state.
- **Drag live reorder vs. measurement**: live reorder uses `SavePosition`-cached row bounds via `calculate_updated_row_index`. Cloud Mode and in-edit rows skip the `Draggable` wrap but still register a `SavePosition` so neighbor-dragged rows can still measure correctly.
