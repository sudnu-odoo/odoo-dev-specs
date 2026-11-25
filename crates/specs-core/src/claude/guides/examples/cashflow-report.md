# Cash flow detail report

**Request.** A cash flow report that shows, for each bank movement, the invoices or bills
it paid, split into tax base, VAT and withholding tax, with the cost center.

**Key decisions**
- A **Tax Type** selection (VAT / WHT) on **tax groups**, so the report knows which taxes
  are VAT and which are withholding, without hard-coding taxes.
- The logic starts from journal items: find the payment, follow the reconciliation
  (matching) to the invoices/bills it paid, and split the paid amount over their lines
  **in proportion** (also with partial payments and write-offs); items paid directly on a
  bank statement without a payment are recorded as they are.
- Each report line: date, bank journal, payment number, related invoice/bill and line,
  analytic (cost center), currency, tax base, VAT, WHT, amount.

**How it was drawn** — numbered parts (01 setup field, 02 logic for invoice/bill
payments, 03 logic for direct bank statements), each case a worked example: the journal
items of invoice → payment → bank, the matching drawn with arrows, the computation step by
step with real-looking amounts, and the expected report as a table. Obsolete approaches
kept but marked OBSOLETE.

**Traps**: one payment for many invoices; write-offs; partial payments; not counting a
payment twice; foreign currency.
