# Custom goods receipt (PDF)

**Request.** A printed goods receipt in the client's own layout, from the receipt
(`stock.picking`), with approval signatures.

**Key decisions**
- Every part of the page is mapped to its source: company name/logo/address, the
  purchase order and its approval date, the purchase request (date, description, cost
  center), the warehouse, a new document number with its own prefix, a delivery note
  number (new text field), remarks.
- Lines from `stock.move`: product code and name, ordered quantity, received before
  (cumulative for the same PO), received now, returned, **remaining = ordered − received
  before − received now − returned**, internal notes.
- Signature boxes following the approval hierarchy (requester, their coach, their manager
  from the employee records); once approved, print "Approved".
- Footer with page number and print date; a field removed from the old layout is crossed
  out.

**How it was drawn** — the client's sample document (as-is) beside the Odoo record; arrows
from each part of the document to the field it comes from, with "new field" marked;
formulas written next to the computed columns.

**Traps**: cumulative quantities across several receipts of the same PO; returns; where
the approvers come from.
