# A new Material Request app

**Request.** Users request materials for a project; after approval the stock comes by
internal transfer, and what is missing is bought through a purchase request. Odoo.sh.

**Key decisions**
- A **new app** with its own menu and models: Material Request, its lines ("List
  Request"), and a configuration model "Material Request Category" (with its approvers,
  archivable).
- A **status field** (statusbar, not clickable) with **separate statuses for each stage**:
  Draft → To Approve → Internal Transfer → Purchase Request → Done, plus Cancelled and
  Rejected. Not one "Approved" status: the stage tells users what is happening. Every button and field states when
  it is visible or read-only **by status** (e.g. Submit only in Draft; Approve/Reject only
  in To Approve; Create ITR only in Internal Transfer).
- One approval step: approvers come from the request category; standard activities, due
  2 days after Submit; approving closes the others' activities, cancel/reject closes all.
- On approval, an internal transfer (out) is created and marked To Do automatically: its
  operation type is found by warehouse + operation category; locations, dates, project and
  products are filled from the request. Demand = requested − already delivered.
- Quantities on each line follow the documents: delivered (ITR out Done), received
  (ITR in Done), returns deducted, PR quantity and PR approved quantity.
- Purchase requests use the standard **Approvals** app (approval.request), filled from the
  request; the purchase order's receipt goes to the request's destination.
- Done automatically when every line is covered: **received** (transfer in done, not just
  delivered out) + approved PR quantity ≥ requested.

**How it was drawn** — four frames:
1. App, menus and models: each new field written next to the form with its type,
   readonly/invisible rule and domain; buttons listed with their visibility rule.
2. Status flow: the form in **each** status (a `scenario` per screen), arrows labelled with
   the button that moves it from one to the next.
3. Internal transfer flow: request → created transfer, each auto-filled field linked by an
   arrow to where it comes from; the quantity formulas written out; a return case.
4. Purchase request flow: same pattern, including the case of a second PR.

**Traps**: several transfers or PRs per request; returns; which statuses count in each
quantity; operation type not found (give a fallback rule).
