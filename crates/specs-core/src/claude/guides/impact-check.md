# Impact check: what else does this change touch?

Read this before proposing any change to existing behaviour: a changed rule, a new step in a flow, a field that changes meaning, something done "before" or "after" what Odoo does. The goal: find what the change breaks or alters that nobody asked about, and design it so it does not.

Search first, conclude second. Never say "no impact" from memory.

## 1. Scale the check to the change
| Change | Check |
|---|---|
| A label, a new field shown only for information | Skip |
| A new isolated field or screen | Quick: does something with that name already exist? |
| A changed rule, flow step, compute, domain, view or access right | Full check (sections 2 to 4) |
| Renaming, removing, or changing what a field means | Full check, plus the client's own setup (section 4) |

## 2. Follow the flow downstream, in the source
Odoo documents chain: one action writes the next document. Trace what the changed step creates or updates, and what reads it after. Typical chains to follow:
- **Stock → valuation → accounting**: a validated stock move creates its valuation (layers) and, with automatic valuation, a journal entry. Its **date**, **quantity** and **cost** come from the move. Changing any of them (a backdated move, a cancelled move, a different cost) changes the valuation and the entry.
- **Sale / purchase → delivery / receipt → invoice → payment**: quantities delivered or received decide what is invoiced; prices and taxes flow to the invoice.
- **Manufacturing**: components consumed and finished products produced make stock moves, hence valuation too.
- **Accounting rules that refuse changes**: the **lock dates** (no entry before them), posted entries that cannot be edited, reconciled lines, fiscal years, tax reports already filed.
- **Costing methods**: with FIFO or average cost, the order and date of moves decide the cost of later ones; a move inserted in the past changes them.
- **Multi-company and multi-currency**: which company and currency each document uses, and the rate at which date.
- **Reports and analysis** built on these documents (stock valuation, aged balances, margins): they change too.

For each step: say what the change alters there, in plain words.

## 3. Who else uses the same thing
Checklist for a rule that blocks or changes a step (go through every line):
customer portal (signing, paying online) · e-commerce orders · duplicating a record ·
import · list / batch actions · mobile and barcode apps · scheduled actions · API.

- Other screens, reports, automations and modules that read the same field, button or rule.
- Other ways the same records are made or moved on: import, duplicating a record, the customer portal (signing, paying online), e-commerce, email, scheduled actions, other apps. A rule that blocks a button must also hold on those paths.
- Edge cases: several records at once, archived or cancelled records, zero quantities, several companies.

## 4. What only the client's database knows
Their own Studio views, automation rules, saved filters and custom fields may also use what changes. You cannot see them (never look at client data): list them as **questions to confirm with the client** ("Do any of your automations or reports use the Delivery Date?").

## 5. Revise the proposal
When the change has side effects, do not just warn: **redesign it** and say why in one or two plain sentences.
Example: "Backdating a delivery would also backdate its stock valuation entry into a closed period, which the lock date refuses, and would change the FIFO cost of later moves. Instead: keep the real date on the move and add an 'Effective date' field shown on the delivery and its report." Then the spec shows the safer design, with a short **Impact** note listing what else changes.
