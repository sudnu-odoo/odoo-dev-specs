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

**How it was drawn** — four frames, each a storyboard of real screens, colours with one
meaning (red new, green user action / status, blue system result, orange condition):
1. Setup: the operation type form with the new Approvers field outlined in red; an
   orange NOTE (only when an approver is set); the transfer in Draft with Validate crossed
   out and the status bar outlined in green; the Ready transfer with Validate (approvers
   only) and the new Reject button.
2. Location lock: the transfer with the new checkbox (red note: administrators only,
   unticked by default, read-only when Done/Cancelled), then "IF Unlock = FALSE" (both
   locations outlined in blue: read-only) and "IF Unlock = TRUE" (outlined in green:
   editable), one screenshot each.
3. Activity creation: orange NOTE on top; "CASE 1: manual" — the Draft transfer (green:
   user clicks Mark as To Do) → the Ready transfer with two "Grant Approval" activities in
   the chatter (blue: created for each approver) and the Schedule Activity pop-up showing
   the due date (red: 2 days after Mark as To Do); "CASE 2: from a sales/purchase order"
   — the transfer with its activities already there (green: due on the scheduled date,
   also for backorders). Flow arrows from the clicked button to what it causes.
4. Approve / reject: from Draft (Mark as To Do), "IF APPROVED" → Ready (status bar
   outlined) → Done with the chatter showing the approver's activity done and the
   other's cancelled; "IF REJECTED" → Reject clicked → back to Draft with all activities
   closed. Each state a screenshot, arrows between them labelled with the click.

**Traps**: backorders and automatic transfers also need the request; closing the other
approvers' activities; standard behaviour unchanged when no approver is set.
