# Configurable contact categorization

**Request.** Classify contacts on several levels (classification, business type, entity
type, and a sub-type that depends on the business type), and let the client maintain the
values themselves.

**Key decisions**
- Each level is a **small configuration model** (name, plus a flag when needed, e.g.
  "Is Transporter" used elsewhere as a domain) and a **many2one on the contact**, instead
  of fixed selection lists, so the client can add values.
- A "Categorization" section under Contacts › Configuration with one menu per model.
- The business type has a **Type** (Company / Government / Joint Operation); the matching
  sub-type field (Company Type, Government Type, Joint Type) is shown only for that type.

**Why many2one, said in the spec**: the client maintains the values, **and** changing a
selection list into a many2one later means migrating every contact's value.

**How it was drawn** — one page, left to right: the configuration models (fields listed),
the new menus, the contact form with the new fields outlined, and a `table` of the values
and of which sub-type field shows for which type (Type → field shown). Earlier version (selection
fields) crossed out with the reason: values must be configurable.

**Traps**: changing a selection to a many2one later loses data, so decide early; the
visibility rule depends on a field of the related record.
