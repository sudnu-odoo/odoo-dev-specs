//! Prototyping: the new fields and models of a spec are really created on the local
//! database (like Studio would, as `x_` data records) just for the screenshots, then
//! removed. So "to be" screens show the new field in its real place.

use anyhow::{ensure, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Field types that can be created as data (what Studio offers).
const TYPES: [&str; 12] = [
    "char",
    "text",
    "html",
    "integer",
    "float",
    "monetary",
    "boolean",
    "date",
    "datetime",
    "selection",
    "many2one",
    "many2many",
];

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct Prototype {
    /// New models, e.g. {"model": "x_approval_matrix", "label": "Approval Matrix"}.
    #[serde(default)]
    pub models: Vec<ProtoModel>,
    /// New fields, on existing models or on the new ones.
    #[serde(default)]
    pub fields: Vec<ProtoField>,
    /// New buttons on an existing form: in the header, or a smart button.
    #[serde(default)]
    pub buttons: Vec<ProtoButton>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProtoButton {
    /// Model of the form, e.g. "sale.order".
    pub model: String,
    /// Text on the button, e.g. "Revise Quotation" or, for a smart button, "Revisions".
    pub label: String,
    /// Header button: the existing button it comes after, e.g. "action_confirm".
    pub after: Option<String>,
    /// A smart button (top right of the form) instead of a header button.
    #[serde(default)]
    pub smart: bool,
    /// Smart button: the example count shown, e.g. "2".
    pub value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProtoModel {
    /// Technical name starting with x_, e.g. "x_approval_matrix". A screen shows its
    /// list with page "model:x_approval_matrix", a new record with "model:x_approval_matrix/new".
    pub model: String,
    /// Name the user sees, e.g. "Approval Matrix".
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProtoField {
    /// Model it goes on, e.g. "sale.order" or a new "x_..." model.
    pub model: String,
    /// Technical name starting with x_, e.g. "x_delivery_deadline".
    pub name: String,
    /// Label the user sees, e.g. "Delivery Deadline".
    pub label: String,
    /// char, text, html, integer, float, monetary, boolean, date, datetime, selection,
    /// many2one or many2many.
    #[serde(rename = "type")]
    pub field_type: String,
    /// For many2one / many2many: the model it points to, e.g. "res.partner".
    pub relation: Option<String>,
    /// For selection: the choices, e.g. ["Draft", "Approved"].
    #[serde(default)]
    pub selection: Vec<String>,
    /// On an existing model: the field it is shown after on the form, e.g. "validity_date".
    pub after: Option<String>,
    /// Also show it as a list column, after this column, e.g. "date_order".
    pub list_after: Option<String>,
}

impl Prototype {
    pub fn is_empty(&self) -> bool {
        self.models.is_empty() && self.fields.is_empty() && self.buttons.is_empty()
    }

    pub fn validate(&self) -> Result<()> {
        let new_models: Vec<&str> = self.models.iter().map(|m| m.model.as_str()).collect();
        for m in &self.models {
            ensure!(
                is_x_name(&m.model),
                "A new model's name starts with x_ and has only lowercase letters, digits, _ and . (not \"{}\").",
                m.model
            );
        }
        for f in &self.fields {
            ensure!(
                is_x_name(&f.name) && !f.name.contains('.'),
                "A new field's name starts with x_ and has only lowercase letters, digits and _ (not \"{}\").",
                f.name
            );
            ensure!(
                TYPES.contains(&f.field_type.as_str()),
                "Field {} has type \"{}\"; use one of: {}.",
                f.name,
                f.field_type,
                TYPES.join(", ")
            );
            if f.field_type.starts_with("many2") {
                ensure!(f.relation.is_some(), "Field {} needs `relation`.", f.name);
            }
            if f.field_type == "selection" {
                ensure!(
                    !f.selection.is_empty(),
                    "Field {} needs `selection`.",
                    f.name
                );
            }
            let on_new = new_models.contains(&f.model.as_str());
            ensure!(
                on_new || f.after.is_some() || f.list_after.is_some(),
                "Field {} is on an existing model: give `after` (the form field it is shown after) or `list_after`.",
                f.name
            );
            for name in [
                Some(&f.model),
                f.relation.as_ref(),
                f.after.as_ref(),
                f.list_after.as_ref(),
            ]
            .into_iter()
            .flatten()
            {
                ensure!(
                    name.chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.'),
                    "\"{name}\" is not a technical name."
                );
            }
        }
        for b in &self.buttons {
            ensure!(
                b.smart || b.after.is_some(),
                "Button \"{}\": give `after` (the header button it comes after, e.g. \"action_confirm\") or set `smart`.",
                b.label
            );
            for name in [Some(&b.model), b.after.as_ref()].into_iter().flatten() {
                ensure!(
                    name.chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.'),
                    "\"{name}\" is not a technical name."
                );
            }
        }
        Ok(())
    }

    /// Script run in the logged-in Odoo page: creates everything and answers a JSON
    /// string {"pages": {"model:x_..": "/web#..."}, "undo": {...}, "error"?: "..."}.
    pub fn setup_script(&self) -> String {
        let args = serde_json::to_string(self).expect("serializable");
        SETUP_JS.replace("__ARGS__", &args)
    }

    /// Script removing what `setup_script` created, given its answer.
    pub fn undo_script(answer: &str) -> String {
        let undo = serde_json::from_str::<serde_json::Value>(answer)
            .map(|v| v["undo"].clone())
            .unwrap_or_default();
        UNDO_JS.replace("__UNDO__", &undo.to_string())
    }
}

fn is_x_name(name: &str) -> bool {
    name.len() > 2
        && name.starts_with("x_")
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '.')
}

const RPC_JS: &str = r#"
  const rpc = async (model, method, args, kwargs) => {
    const r = await fetch('/web/dataset/call_kw/' + model + '/' + method, {
      method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ jsonrpc: '2.0', method: 'call',
        params: { model, method, args, kwargs: kwargs || {} } }) });
    const j = await r.json();
    if (j.error) throw new Error((j.error.data && j.error.data.message) || j.error.message);
    return j.result;
  };
"#;

const SETUP_JS: &str = r#"(async () => {
  const P = __ARGS__;
  __RPC__
  const undo = { actions: [], views: [], server: [], access: [], fields: [], models: [] };
  const esc = (t) => String(t).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
  const pages = {};
  // A view type is "list" since Odoo 17 and "tree" before.
  const view = async (vals) => {
    try { return await rpc('ir.ui.view', 'create', [vals]); }
    catch (e) {
      if (vals.type !== 'list') throw e;
      return await rpc('ir.ui.view', 'create', [{ ...vals, type: 'tree',
        arch: vals.arch.replace(/^<list/, '<tree').replace(/list>$/, 'tree>') }]);
    }
  };
  try {
    for (const m of P.models) {
      const id = await rpc('ir.model', 'create', [{ name: m.label, model: m.model, state: 'manual' }]);
      undo.models.push(id);
      // Internal users may use it: "ir.model.access" until Odoo 19, "ir.access" after.
      const group = (await rpc('ir.model.data', 'search_read', [[['module', '=', 'base'],
        ['name', '=', 'group_user']]], { fields: ['res_id'] }))[0].res_id;
      try {
        undo.access.push(['ir.model.access', await rpc('ir.model.access', 'create', [{ name: 'prototype ' + m.model,
          model_id: id, group_id: group, perm_read: true, perm_write: true, perm_create: true, perm_unlink: true }])]);
      } catch (e) {
        undo.access.push(['ir.access', await rpc('ir.access', 'create', [{ name: 'prototype ' + m.model,
          model_id: id, group_id: group, operation: 'crud' }])]);
      }
    }
    for (const f of P.fields) {
      const ids = await rpc('ir.model', 'search', [[['model', '=', f.model]]]);
      if (!ids.length) throw new Error('There is no model ' + f.model + ' on this database (is its module installed?).');
      const vals = { model_id: ids[0], name: f.name, field_description: f.label, ttype: f.type, state: 'manual' };
      if (f.relation) vals.relation = f.relation;
      if (f.type === 'selection')
        vals.selection_ids = f.selection.map((s, i) => [0, 0, { value: s.toLowerCase().replace(/\W+/g, '_'), name: s, sequence: i }]);
      undo.fields.push(await rpc('ir.model.fields', 'create', [vals]));
    }
    const isNew = (model) => P.models.some((m) => m.model === model);
    const parentOf = async (model, type, what) => {
      let parents = [];
      for (const t of type === 'list' ? ['list', 'tree'] : [type]) {
        if (parents.length) break;
        try {
          parents = await rpc('ir.ui.view', 'search', [[['model', '=', model], ['type', '=', t],
            ['mode', '=', 'primary'], ['inherit_id', '=', false]]], { order: 'priority, id', limit: 1 });
        } catch (e) { /* "tree" is not a view type any more */ }
      }
      if (!parents.length) throw new Error('No ' + type + ' view of ' + model + ' to add ' + what + ' to.');
      return parents[0];
    };
    const extend = async (f, type, after) => {
      const parent = await parentOf(f.model, type, f.name);
      undo.views.push(await rpc('ir.ui.view', 'create', [{ name: 'prototype ' + f.name, model: f.model,
        inherit_id: parent, mode: 'extension', priority: 99,
        arch: '<xpath expr="//field[@name=\'' + after + '\']" position="after"><field name="' + f.name + '"' +
          (type === 'list' ? ' optional="show"' : '') + '/></xpath>' }]));
    };
    for (const f of P.fields.filter((f) => !isNew(f.model))) {
      if (f.after) await extend(f, 'form', f.after);
      if (f.list_after) await extend(f, 'list', f.list_after);
    }
    // A button does nothing: it is only there for the screenshot.
    for (const b of P.buttons) {
      const ids = await rpc('ir.model', 'search', [[['model', '=', b.model]]]);
      if (!ids.length) throw new Error('There is no model ' + b.model + ' on this database (is its module installed?).');
      const act = await rpc('ir.actions.server', 'create', [{ name: 'prototype ' + b.label, model_id: ids[0],
        state: 'code', code: 'pass' }]);
      undo.server.push(act);
      const arch = b.smart
        ? '<xpath expr="//div[@name=\'button_box\']" position="inside"><button name="' + act +
          '" type="action" class="oe_stat_button" icon="fa-files-o"><div class="o_stat_info">' +
          '<span class="o_stat_value">' + esc(b.value || '') + '</span><span class="o_stat_text">' + esc(b.label) +
          '</span></div></button></xpath>'
        : '<xpath expr="(//header/button[@name=\'' + b.after + '\'])[last()]" position="after"><button name="' + act +
          '" type="action" string="' + esc(b.label) + '"/></xpath>';
      undo.views.push(await rpc('ir.ui.view', 'create', [{ name: 'prototype button ' + b.label, model: b.model,
        inherit_id: await parentOf(b.model, 'form', 'the button ' + b.label), mode: 'extension', priority: 99, arch }]));
    }
    for (const m of P.models) {
      const fields = ['x_name', ...P.fields.filter((f) => f.model === m.model).map((f) => f.name)];
      const tags = fields.map((n) => '<field name="' + n + '"/>').join('');
      undo.views.push(await view({ name: 'prototype ' + m.model + ' list', model: m.model, type: 'list',
        arch: '<list>' + tags + '</list>' }));
      undo.views.push(await view({ name: 'prototype ' + m.model + ' form', model: m.model, type: 'form',
        arch: '<form><sheet><group>' + tags + '</group></sheet></form>' }));
      const action = await rpc('ir.actions.act_window', 'create', [{ name: m.label, res_model: m.model, view_mode: 'list,form' }]);
      undo.actions.push(action);
      pages['model:' + m.model] = '/web#action=' + action + '&model=' + m.model + '&view_type=list';
      pages['model:' + m.model + '/new'] = '/web#action=' + action + '&model=' + m.model + '&view_type=form';
    }
  } catch (e) {
    return JSON.stringify({ pages, undo, error: String((e && e.message) || e) + ' (on ' + location.href + ')' });
  }
  return JSON.stringify({ pages, undo });
})()"#;

const UNDO_JS: &str = r#"(async () => {
  const U = __UNDO__ || {};
  __RPC__
  const errors = [];
  const steps = [['ir.actions.act_window', U.actions], ['ir.ui.view', U.views], ['ir.actions.server', U.server],
    ...(U.access || []).map(([model, id]) => [model, [id]]),
    ['ir.model.fields', U.fields], ['ir.model', U.models]];
  for (const [model, ids] of steps) {
    if (!ids || !ids.length) continue;
    try { await rpc(model, 'unlink', [ids.slice().reverse()]); }
    catch (e) { errors.push(String((e && e.message) || e)); }
  }
  return JSON.stringify(errors);
})()"#;

/// The scripts with the RPC helper inlined.
pub fn setup_js(p: &Prototype) -> String {
    p.setup_script().replace("__RPC__", RPC_JS)
}

pub fn undo_js(answer: &str) -> String {
    Prototype::undo_script(answer).replace("__RPC__", RPC_JS)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(name: &str, ty: &str) -> ProtoField {
        ProtoField {
            model: "sale.order".into(),
            name: name.into(),
            label: "L".into(),
            field_type: ty.into(),
            relation: None,
            selection: vec![],
            after: Some("validity_date".into()),
            list_after: None,
        }
    }

    #[test]
    fn names_must_be_x_and_types_known() {
        let ok = Prototype {
            fields: vec![field("x_deadline", "date")],
            ..Default::default()
        };
        ok.validate().unwrap();
        for bad in [
            field("deadline", "date"),
            field("x_d", "json"),
            field("x_partner", "many2one"),
        ] {
            let p = Prototype {
                fields: vec![bad],
                ..Default::default()
            };
            assert!(p.validate().is_err());
        }
        let mut injected = field("x_d", "date");
        injected.after = Some("a']/..|//x[@n='".into());
        assert!(Prototype {
            fields: vec![injected],
            ..Default::default()
        }
        .validate()
        .is_err());
    }

    #[test]
    fn a_field_on_an_existing_model_says_where_it_goes() {
        let mut f = field("x_d", "date");
        f.after = None;
        let p = Prototype {
            fields: vec![f.clone()],
            ..Default::default()
        };
        assert!(p.validate().unwrap_err().to_string().contains("after"));
        f.model = "x_matrix".into();
        let p = Prototype {
            models: vec![ProtoModel {
                model: "x_matrix".into(),
                label: "Matrix".into(),
            }],
            fields: vec![f],
            buttons: vec![],
        };
        p.validate().unwrap();
    }

    fn button(after: Option<&str>, smart: bool) -> ProtoButton {
        ProtoButton {
            model: "sale.order".into(),
            label: "Revise <Quotation>".into(),
            after: after.map(Into::into),
            smart,
            value: None,
        }
    }

    #[test]
    fn a_button_says_where_it_goes() {
        let p = |b| Prototype {
            buttons: vec![b],
            ..Default::default()
        };
        p(button(Some("action_confirm"), false)).validate().unwrap();
        p(button(None, true)).validate().unwrap();
        assert!(p(button(None, false))
            .validate()
            .unwrap_err()
            .to_string()
            .contains("after"));
        assert!(p(button(Some("x']|//y"), false)).validate().is_err());
    }

    #[test]
    fn button_labels_are_escaped_and_undone() {
        let js = setup_js(&Prototype {
            buttons: vec![button(Some("action_confirm"), false)],
            ..Default::default()
        });
        assert!(js.contains("Revise <Quotation>") && js.contains("esc(b.label)"));
        assert!(undo_js(r#"{"undo":{"server":[4]}}"#).contains("ir.actions.server"));
    }

    #[test]
    fn scripts_carry_the_plan_and_the_undo_list() {
        let p = Prototype {
            models: vec![],
            fields: vec![field("x_deadline", "date")],
            buttons: vec![],
        };
        let js = setup_js(&p);
        assert!(js.contains("\"x_deadline\"") && js.contains("call_kw") && !js.contains("__"));
        let undo = undo_js(r#"{"pages":{},"undo":{"fields":[7],"views":[9]}}"#);
        assert!(undo.contains("\"fields\":[7]") && !undo.contains("__UNDO__"));
        // An unreadable answer undoes nothing rather than failing.
        assert!(undo_js("garbage").contains("const U = null"));
    }
}
