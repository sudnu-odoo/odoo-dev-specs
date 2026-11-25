# Lessons from comparing specs with good ones

Short rules learned when the app's specs were compared with the ones the team made.

- A requirement that is a **flow** (states, approvals, automatic documents) is drawn as a
  storyboard: one real screenshot per state, including where each branch ends (Done, back
  to Draft), branches side by side (approved / rejected, manual / automatic), arrows
  labelled with the click or event that causes the next state.
- When a rule applies only under a condition (e.g. "only when an approver is set"), say
  it once at the top of the frame as a NOTE.
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
