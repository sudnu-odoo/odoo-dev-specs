# Odoo Online: what is possible

The client runs on Odoo Online (SaaS): **no custom Python code** and no third-party apps. A solution is a small module made only of data, which Odoo imports.

## Possible
- **New fields and models** (named `x_...`, e.g. `x_delivery_window`), with computed fields written as a small formula.
- **Views**: form, list, kanban, search, pivot, graph; menus and actions.
- **Automation rules** (on create, on save, when a field changes, on a schedule) that run **server actions**: small Python snippets run by Odoo, with limited access (no imports, no changing Odoo's own methods).
- **Access rights, groups and record rules.**
- **Reports (QWeb) and email templates.**
- **Interface changes in JavaScript** (shipped in the imported module), when no server code is needed behind them.

## Not possible (say why, in plain words)
- **Changing what Odoo's own buttons or methods do** (what Confirm, Validate or Post do inside). Only a check before or an action after, with an automation rule; or act on the **data** the standard reads so it takes the other path.
- **Changing a report built as a database query** (most analysis reports): its calculation and what you can group or sum cannot change. Extra information can only be shown.
- Field-level access groups, chatter tracking changes, restricting export, new Python libraries, website controllers.

## When it gets fragile
A workaround is fragile when it replaces what one code change would do: a chain of automation rules, a scheduled action that checks what a button should do, a standard button replaced by a server action, standard behaviour rebuilt because it cannot be reached. Then say so plainly: what works and what is fragile. Do not mention what Odoo.sh could do unless the user asks: the hosting is the client's choice.

Do not mention Studio and never use `x_studio_` names.
