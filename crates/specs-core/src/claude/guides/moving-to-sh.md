# A client moving from Odoo Online to Odoo.sh

When the client moves their Odoo Online database to Odoo.sh, their data-only customisations are usually turned into a real module. This guide is about what that means for them, not how the developers do it.

## What changes for the client
- **Fields and models keep their data**: their `x_` names become normal names (e.g. `x_delivery_window` becomes `delivery_window`). Saved filters, reports and exports that use the old names must be checked.
- **Automation rules become code**: the result should be the same, but some details can differ, and these must be said and tested:
  - a check that used to show an error may show a slightly different message;
  - something that ran **after saving** can now run **while saving**, so the order with other steps can change;
  - imports and the customer portal may now go through the same logic.
- **What was a workaround on Odoo Online can now be done the clean way** (e.g. changing what a button does). List those as improvements, the client decides.
- **Their own Studio views, automations and filters** stay in the database: they must be listed with the client and checked, as they may use the old names.

## What to give the developers and the client
- The list of customisations to carry over, in business words (what each does, where).
- What will behave differently, and what to test after the move (the main flows, with a few real-looking examples on a test database, never their production data).
- Questions to confirm with the client.

No time or effort estimates: that is the technical team's job.
