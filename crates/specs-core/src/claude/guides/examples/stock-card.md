# Stock card report

**Request.** A report of stock movements per product over a period: opening balance,
quantities and values in and out per kind of operation, closing balance; exportable to
Excel. Odoo.sh.

**Key decisions**
- A required **"Operation Category"** selection on the operation type (Receipt, Delivery,
  Internal in/out, Usage, Adjustment…), so each move can be put in the right column
  without hard-coding operation types.
- A new menu Inventory › Reporting › Stock Card, built on `stock.move`.
- Rules: Done moves only; storable products only; only the companies selected in the
  company switcher; (to confirm) perpetual valuation categories only.
- Opening quantity and value use the **stock at date** logic at the start date. Closing
  balance = opening + receipts − deliveries ± internal ± transfers − usage ± adjustments;
  adjustments signed by whether the inventory loss location is source or destination.
- Filters: dates (start 00:00, end 23:59), locations including their children, product
  categories, products, lot/serial "contains"; filters can be saved.
- Grouped by product; folded rows already show the totals per category, unfolded shows the
  moves. Default when opened: **today** (00:00–23:59, not the current month), grouped by
  product. Button to export to XLSX.

**How it was drawn** — numbered parts, **each its own section**: 1 setup field, 2 menu, PART 1 RULES,
PART 2 FILTERS, PART 3 FIELDS & BUTTONS, DEFAULTS. A mock-up of the expected report (a
table with the columns) next to the real screens, each column labelled with its source
and formula.

**Traps**: opening balance at a date (not today's stock); child locations; signs of
adjustments; multi-company.
