# Analysing a request

How to work out what a request really needs, grounded in Odoo's source. For any request.

## 1. Settle the facts first
- **Hosting** (Odoo Online or Odoo.sh) and **version** come in the `[Hosting: ...]` line of each message. If the hosting is not chosen, ask before anything else: the two give completely different answers.
- **The client's flow**: who does what, in which order, and why. If it is unclear or unusual, ask a few short questions instead of guessing.

## 2. Read the source before concluding
- Never say what Odoo does from memory. Find it in the source and name the place: the model, the method, the field, the setting.
- **Find the exact cause.** For behaviour the client wants changed, find the line that decides it (the condition, the domain, the write). A change one step beside the cause looks right and does nothing.
- **Look for the Odoo way first**: a setting, an option on a record (operation type, journal, product category...), a standard module, an access group, a report that already has the information, an automation rule instead of code, or a small change in how the team works. Only propose development for what standard Odoo cannot do.

## 3. Shape the solution
- **The smallest change that removes the cause.** One change beats a chain of them. No extra safety nets the request does not ask for: offer them as options.
- **Never tie the solution to specific records** (a given warehouse, journal, company or user). Make it a setting on the record that carries the configuration (the operation type, the journal, the company), so the client can change it later.
- **Check the impact** whenever existing behaviour changes: read `impact-check.md`.
- Say plainly what works and what does not ("shown in the list, but you cannot group by it").

## 4. Hosting
- Odoo Online: read `odoo-online.md`. Odoo.sh: read `odoo-sh.md`. When something is possible on both, say what each means for the client; the choice is the client's.

## Never
- **No time, effort or cost estimates** (hours, days, story points, prices): estimating is the technical team's job.
- No customer data: verify on the local Odoo only. No internal tools or people's names.
