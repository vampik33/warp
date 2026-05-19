# Queued Prompts UI

## Summary
Replace the single-slot pending-prompt indicator with a collapsible "queued prompts" panel that supports multiple prompts, in-place editing, drag-to-reorder, and per-row delete.
Queued prompts run sequentially as the agent finishes each preceding exchange.
## Problem
Today only one follow-up prompt can be queued (`/queue`, the auto-queue toggle, `/compact-and`, `/fork-and-compact`).
Re-queueing replaces the previous prompt, the user can't reorder or edit what's pending, and there's no way to deal with multiple in-flight ideas without losing work.
## Goals
- Let users queue any number of follow-up prompts while the agent is responding, and have them auto-fire in order.
- Make the queued prompts visible, reorderable, editable, and individually removable from a single panel.
- Preserve the existing trigger surfaces (auto-queue toggle and `/queue` slash command) and merge other "queue this and fire after" features (`/compact-and`, `/fork-and-compact`, Cloud Mode initial prompt) into the same panel.
- Reuse the existing trigger-surface flags and the existing `PendingUserQueryIndicator` flag for the visual panel — no new flags introduced.
## Non-goals
- Persisting the queue across app restarts.
- Cross-conversation queueing (the queue belongs to the conversation it was filed against).
- Reordering or editing the prompt that's currently executing (only items still pending in the queue are editable).
- A "Send now" affordance to interrupt the in-flight exchange (explicitly removed; users cancel via the existing stop button if they want to fire a queued prompt earlier).
## Behavior
### Queue panel placement and visibility
1. The queue panel renders between the warping indicator (status bar) and the agent input box, anchored to the bottom of the conversation area, in the same vertical slot the inline menu uses when it's open.
2. The panel is visible whenever the active conversation has at least one queued prompt; otherwise the panel is not rendered (no empty state).
3. The panel has a header `"<N> queued"` with a chevron icon. The body of the panel (everything below the header) is what collapses, not the header itself. Clicking the chevron (or anywhere on the header) toggles the panel between expanded (header + rows visible) and collapsed (only the header is visible). Default state is expanded. The collapsed state persists for the lifetime of the queue (across re-orderings, edits, deletions, additions). Adding a new prompt while collapsed does not auto-expand.
4. `/queue` and the auto-queue toggle continue to be gated by `QueueSlashCommand`. `/compact-and` continues to be gated by `SummarizationConversationCommand`, and `/fork-and-compact` follows the existing fork-command availability. The visual queue panel itself is gated by the existing `PendingUserQueryIndicator` flag (the same flag that today gates the single-slot pending block).
### What gets queued
5. The auto-queue toggle in the warping indicator keeps the same semantics: when on, any prompt the user submits while the active conversation is in progress (`InProgress` or `Blocked`) is appended to the queue rather than sent. When off, regular submits still cancel-and-resend (existing behavior).
6. `/queue <prompt>` appends `<prompt>` to the queue when the active conversation is in progress, and behaves like a normal send when the conversation is idle (existing semantics).
7. `/compact-and <prompt>` and `/fork-and-compact <prompt>` append `<prompt>` to the queue tagged so that summarization (or fork-then-summarization) runs before `<prompt>` fires. Both render as ordinary queue rows with the same edit, delete, and reorder affordances as any other queued prompt. Additional prompts queued via `/queue` or the auto-queue toggle stack behind them and fire in queue order, exactly like prompts queued behind a regular `/queue` entry — the only difference for `/compact-and` / `/fork-and-compact` rows is that the corresponding summarize-or-fork action runs before that specific row's prompt is sent.
8. For Cloud Mode runs (both Oz and third-party harness), the submitted prompt appears in the queue panel as the first queue item while the run is starting up; it is not editable or removable (its lifecycle is owned by the cloud mode setup flow, not the user). For non-Oz harnesses the row is torn down when the harness CLI starts; for Oz runs it's torn down when the first agent exchange streams back from the cloud session.
9. Submitting in shell mode (input type is Shell, not AI) is never queued — it runs in the terminal as today, regardless of toggle state or in-progress AI status.
10. `/queue` with an empty argument shows an error toast and does not modify the queue (existing behavior).
11. The queue is per-conversation. Switching to another conversation hides the panel and shows that conversation's queue (which may be empty).
### Queue rows
12. Each queue row shows, left to right:
   - A drag handle icon (six-dot grid).
   - The prompt text (single-line, truncated with ellipsis when it overflows the row).
   - On hover: a pencil (edit) and a trash (delete) icon-button, right-aligned.
13. Hovering a row reveals the edit/delete icons. Moving the cursor off the row hides them.
14. Cloud Mode initial-prompt rows do not show the edit or delete icons even on hover, since they are not user-editable.
15. Rows render in queue order from top (next to fire) to bottom (last to fire).
### Edit interaction
16. Clicking the pencil icon on a row replaces the row's static text with an inline single-line editor pre-filled with the current prompt text, identical to the tab-rename interaction.
17. Pressing Enter commits the edit (the row's prompt is replaced with the editor contents) and exits edit mode. An empty edit restores the original prompt text and exits edit mode.
18. Pressing Escape cancels the edit and restores the original prompt text. Clicking outside the row, including focusing the main input, commits the current editor text.
19. While a row is in edit mode, that row's drag handle is inert (the row cannot be reordered until the edit is committed or cancelled). Other rows can still be dragged.
20. Only one row can be in edit mode at a time. Clicking the pencil on a different row commits any in-progress edit on the previous row, then enters edit mode on the new one.
21. Auto-fire never sends a row that is currently in edit mode. If the active conversation reaches `FinishReason::Complete` while the first queue row is in edit mode, that row is removed from the queue, and — mirroring the delete behavior in (23)–(24) — the row's prompt text (its last committed value, not any uncommitted text still in the inline editor buffer) is placed in the main input box (and the input is focused) only if the input is currently empty. If the input is non-empty, the row is still removed from the queue but its text is discarded; the input is not modified. Other queue rows are not affected and resume normal sequential firing on the next completion. If the row being edited is not the first row, auto-fire proceeds normally for the actual first row; the edited row is left in place and can become the next-to-fire after rows ahead of it drain.
### Delete interaction
22. Clicking the trash icon on a row removes that row from the queue.
23. If the input box is empty when a row is deleted, the deleted row's prompt text is placed in the input (replacing the empty buffer); the input gains focus.
24. If the input box is non-empty when a row is deleted, the deleted prompt is discarded — the input is not modified.
25. Deleting the last visible row in the queue removes the panel (since the queue is now empty); the collapsed/expanded state resets for any future queue.
### Drag-to-reorder
26. Pressing on a row's drag handle and dragging it vertically reorders the row within the queue. The row being dragged renders with reduced opacity (consistent with other drag affordances in the app) while a drop indicator shows where the row will land between two other rows.
27. Dropping the row commits the new order. The first row of the new order is what will fire next.
28. Dragging is constrained to the vertical axis — horizontal motion does not change order.
29. Dragging a row past the top or bottom of the panel scrolls the panel if the queue is taller than its visible bounds.
30. Cloud Mode initial-prompt rows cannot be reordered: their drag handle is inert and other rows cannot be dropped above them.
### Sequential firing
31. When the active conversation reaches `FinishReason::Complete`, the first prompt in the queue is removed and submitted as the next user query in the same conversation, routed through the normal submission path so slash, skill, and session-sharing paths are handled correctly.
32. While that newly-fired prompt is mid-exchange, the rest of the queue stays intact, the panel updates the count to `<N-1> queued`, and additional prompts can still be queued at the tail.
33. The cycle continues until either the queue is empty or one of the abort conditions in (34) fires.
### Cancellation and error handling
34. When the active conversation finishes for any non-`Complete` reason — `Error`, `Cancelled`, `CancelledDuringRequestedCommandExecution` — auto-fire pauses immediately. The queue is not flushed.
35. When auto-fire pauses for one of those reasons:
   - If the input is currently empty and the first queued prompt is user-managed, that prompt is removed from the queue, its text is placed in the input box, and the input gains focus. The row is removed in this case so that re-submitting the input does not also re-fire the same prompt from the queue. Harness-owned Cloud Mode rows are left in place.
   - If the input is non-empty, the first prompt's text is not placed in the input and the queue is left intact (the first prompt remains in the queue at position 0).
   - In both cases all queue rows beyond the first remain intact in the panel, so the user can review, edit, reorder, delete, or send further prompts.
36. Auto-fire resumes naturally the next time the active conversation reaches `FinishReason::Complete` — i.e. the user re-runs or sends a new prompt that succeeds, and from that completion onward the queue resumes draining from the top.
37. Manually cancelling the in-progress agent (stop button or `Ctrl-C` shortcut) is treated as `Cancelled` for the purposes of (34)–(35): the queue pauses, no rows are dropped.
### Conversation lifecycle interactions
38. Exiting the agent view (Esc to terminal, closing the tab/pane) discards the queue for that conversation; switching back later does not restore it.
39. Starting a new conversation clears the queue.
40. The queue belongs to a conversation; if the agent splits the conversation (`/fork`, `/fork-and-compact`), the new conversation starts with an empty queue except for any explicit `initial_prompt` that the fork itself injects.
### Focus
41. The auto-queue toggle keybinding (`Cmd-Shift-J` / `Ctrl-Shift-J`) is unchanged.
42. Submitting from the main input always returns focus to the main input, even when the submission appended to the queue.
### Telemetry
43. Existing `/queue` and auto-queue telemetry events continue to fire. Queue-panel-specific interactions (edit committed, row deleted, row reordered, panel collapsed/expanded) are tracked as new telemetry events so we can measure usage of the new affordances.
