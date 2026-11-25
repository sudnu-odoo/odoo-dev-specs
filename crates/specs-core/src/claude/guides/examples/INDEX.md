# Example specs

Good specs made for past clients, one card each: what was asked, the design decisions,
and how the spec was drawn. Before designing a spec, look here; when one is close to the
request, read its card (at most two) and follow its decisions and layout where they fit.
Never copy client names or data from a card; check every field and model in the source.

- operation-approval.md — approvers on operation types; Validate blocked until approved; approval activities per approver; approve/reject flow; lock fields per user group (Inventory)
- material-request.md — a new app with its own model, menu, status flow and buttons; approval step; creates internal transfers and purchase requests; quantities tracked across documents (Inventory, Purchase, Approvals)
- stock-card.md — a stock movement report: opening balance, in/out per operation category, closing balance; filters; Excel export (Inventory reporting)
- contact-categorization.md — configurable classifications on contacts: small configuration models used as dropdowns, menus under Configuration, fields shown depending on another (Contacts)
- goods-receipt-pdf.md — a custom printed document (PDF) for receipts: each part of the page mapped to its source field, signature blocks, computed quantities (Inventory reports)
- cashflow-report.md — a cash flow detail report built from payments and their reconciled invoices/bills, with VAT/WHT split by a tax type on tax groups (Accounting)
