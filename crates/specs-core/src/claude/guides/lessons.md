# Lessons from comparing specs with good ones

Short rules learned when the app's specs were compared with the ones the team made.

- A requirement that is a **flow** (states, approvals, automatic documents) is drawn as a
  storyboard: one real screenshot per state, including where each branch ends (Done, back
  to Draft), branches side by side (approved / rejected, manual / automatic), arrows
  labelled with the click or event that causes the next state.
- When a rule applies only under a condition (e.g. "only when an approver is set"), say
  it once at the top of the section as an orange NOTE (the section's `note`).
- Colours mean the same in every spec: red = new or changed, green = what the user does
  or a status, blue = what the system does as a result, orange = a condition or a case.
- A condition with two outcomes gets one screenshot per outcome, each labelled with its
  `condition` ("IF Unlock = FALSE" / "IF Unlock = TRUE"), side by side or one below
  the other.
- In a storyboard, each state is a real screen (a `scenario` puts a copy of the record
  in it): the status bar outlined, the chatter showing the activities, the pop-up a click
  opens. Flow arrows go from the button clicked to what changed on the next screen.
- For a new model or app: write each new field next to the form with its type and its
  visible/read-only rule (often "by status"), and list each button with when it shows.
- For a new report or list: the section's `table` (a mock-up of the expected result),
  each column with its source or formula and a few made-up rows; rules, filters and
  defaults as separate numbered parts.
- For a printed document (PDF): today's report with a note on each change, and each
  computed value's formula in the note next to its column, not in a separate section.
- Check each formula with a made-up example before writing it, and say what a return or
  a cancellation does to it; when the sign is a business choice, ask the client.
- For computations: a worked example with real-looking numbers, step by step.
- Prefer configuration over hard-coding: a field on the operation type, tax group or
  category that the client can set, instead of naming specific records.
- Reuse standard features (activities, approvals, stock at date) and say so.
- When a close example card gives a decision (statuses, a default, a field), follow it
  unless the request says otherwise; if you choose differently, say why in one line.
- A mapping or a list of values (type → field shown, status → buttons) goes in a `table`,
  not in a note.
- For accounting logic, the worked example shows the journal items themselves (account,
  debit, credit, matching) as a `table`, then the result.
- A report described in parts (setup, menu, rules, filters, columns, defaults) gets one
  section per part.
