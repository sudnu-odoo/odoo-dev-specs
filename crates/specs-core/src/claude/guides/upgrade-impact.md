# A client upgrading to a newer Odoo version

What the client's customisations and ways of working hit when moving from one version to another. This guide is about the business impact, not how developers migrate code.

## 1. Compare the two versions in the source
The source of each Odoo version on this computer is in a folder named by its version (e.g. `.../19.0/odoo` and `.../20.0/odoo`). For each feature the client uses:
- Find it in both versions: the model, the fields, the screen, the setting.
- Note what was **renamed, moved, removed or replaced** (a field gone, a setting moved to another menu, a flow done differently). Say it in plain words, with where it is now.
- If the target version's source is not on this computer, say so: it can be added in Settings → Odoo versions.

## 2. For each customisation
- Does what it builds on still exist? If the standard feature it relied on was removed, **propose how to keep the client's behaviour** (often the new version has a standard way, which may replace the customisation).
- Does the new version already do it? Then the customisation may be dropped: say so, it is good news.
- Use `impact-check.md` for anything that changes a flow (stock, valuation, accounting).

## 3. Give a clear picture
- What works the same, what changes for users (new screens, new steps), what must be redone, what can be dropped.
- What to test on a test copy after the upgrade (the main flows), and questions to confirm with the client.

No time or effort estimates: that is the technical team's job. Use only the client's version for normal questions; compare versions only for an upgrade.
