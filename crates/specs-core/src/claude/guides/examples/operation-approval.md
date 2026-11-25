# Approval before validating transfers

**Request.** Some operation types (e.g. Receipts) need one or more approvers before a
transfer can be validated. Separately, source/destination locations are locked for
warehouse users on all transfers. Odoo.sh (custom module).

**Key decisions**
- Approvers: a many2many to users **on the operation type** (`stock.picking.type`), not on
  each transfer. Empty = nothing changes: say it once, in a NOTE at the top of each frame.
- No separate Approve button: an approver approves by **validating**. Non-approvers do not
  see Validate; approvers also get a **Reject** button.
- Approval requests are **standard activities** ("Grant Approval"), one per approver:
  - transfer made by hand: created on "Mark as To Do", due 2 days later;
  - transfer from a sales/purchase order **and its backorders**: created automatically,
    due on the transfer's scheduled date.
- When one approver validates, their activity is done and the others' are closed;
  Reject sends the transfer back to Draft and closes all of them.
- Location lock: a **checkbox** "Unlock Source/Destination Location" on the transfer,
  visible to Inventory Administrators only, unticked by default, read-only when
  Done/Cancelled. Unticked = both location fields read-only.

**How it was drawn** — four frames, each a storyboard of real screens:
1. Setup: operation type form with the new field outlined in red; the transfer in Draft
   with Validate crossed out; the Ready transfer with Validate/Reject for approvers.
2. Activity creation: two cases side by side (manual vs from SO/PO), each a chain of
   screenshots (Draft → activity pop-up → transfer with the activities in the chatter),
   arrows from the click to its result, the due-date rule written next to the activity.
3. Approve vs reject: two branches from the same Ready transfer, one screenshot per state
   (Ready → Done, or back to Draft), the chatter showing which activities closed.
4. Location lock: the transfer with the checkbox unticked (locations read-only, outlined)
   and ticked (editable).

**Traps**: backorders and automatic transfers also need the request; closing the other
approvers' activities; standard behaviour unchanged when no approver is set.
