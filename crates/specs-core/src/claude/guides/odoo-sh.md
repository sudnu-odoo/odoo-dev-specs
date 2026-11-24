# Odoo.sh: what is possible

The client runs on Odoo.sh: a **custom module in Python** is possible. New models and fields have normal names (e.g. `delivery_window`, no `x_`).

## Possible
- New models and fields, constraints, computed fields.
- **Changing Odoo's own methods** (what a button does, how a value is computed), keeping the standard behaviour around the change.
- Reports with their own database query, website pages and controllers, scheduled jobs, Python libraries.
- Interface changes in JavaScript.

## Still prefer the standard way
Even when everything can be developed: first a setting, an option, a standard module (read `analysis.md`). Development is for what standard Odoo cannot do.

## Say when it is risky or heavy
- **Changing a core calculation** (taxes, valuation, reconciliation, costing): every later document depends on it. Read `impact-check.md`.
- **Changing a report's database query**: easy to get totals wrong.
- **Many screens or flows touched at once.**
- Changes that every **Odoo upgrade** will have to carry: the more of Odoo's own code a change touches, the more upgrade work later.
