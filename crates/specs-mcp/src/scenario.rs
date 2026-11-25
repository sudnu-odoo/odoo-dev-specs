//! Scenarios: a screen of a storyboard shows a record in a given state (Ready, Done,
//! with its approval activities…). Before the screenshot, a copy of the demo record is
//! made and put in that state through Odoo's own methods; the original never changes,
//! and the copy is removed afterwards when Odoo allows it.

use anyhow::{bail, ensure, Result};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{json, Map, Value};

#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
pub struct Scenario {
    /// Steps on the copy, in order.
    #[serde(default)]
    pub steps: Vec<Step>,
    /// Once the page is open, click the button with this text, e.g. to show a pop-up
    /// ("Mark as Todo" opening "Schedule Activity").
    #[serde(default)]
    pub click: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(tag = "do", rename_all = "snake_case")]
pub enum Step {
    /// Call one of the record's buttons, e.g. "action_confirm", "action_assign",
    /// "button_validate", "action_cancel".
    Call { method: String },
    /// Set field values, e.g. {"scheduled_date": "2026-12-01 10:00:00"}.
    Write { values: Map<String, Value> },
    /// Schedule a to-do activity on the record, for these users (their names; none =
    /// the logged-in user), due in `days` days.
    Activity {
        summary: String,
        #[serde(default)]
        users: Vec<String>,
        #[serde(default)]
        days: Option<i64>,
    },
    /// Mark the record's activities as done (shown in the chatter).
    ActivitiesDone,
}

/// The record id at the end of a page like "/odoo/action-stock.action_picking_tree_all/7".
pub fn record_id(page: &str) -> Option<u64> {
    let digits: String = page
        .trim_end_matches('/')
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.chars().rev().collect::<String>().parse().ok()
}

pub fn validate(model: Option<&str>, page: &str, s: &Scenario) -> Result<()> {
    if s.steps.is_empty() {
        return Ok(());
    }
    ensure!(
        model.is_some_and(|m| !m.is_empty()),
        "A screen with scenario steps needs its `model`."
    );
    ensure!(
        record_id(page).is_some(),
        "A screen with scenario steps needs a page ending with a record id, e.g. /odoo/inventory/receipts/7 (not {page})."
    );
    for step in &s.steps {
        if let Step::Call { method } = step {
            if !method
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_')
                || method.starts_with('_')
            {
                bail!("\"{method}\" is not a button's method name.");
            }
        }
    }
    Ok(())
}

/// Script for the logged-in page: copies the record, runs the steps on the copy and
/// answers a JSON string {"page": the copy's page, "copy": id} or {"error": "…"}.
pub fn script(model: &str, page: &str, s: &Scenario) -> String {
    let id = record_id(page).unwrap_or_default();
    let steps: Vec<Value> = s
        .steps
        .iter()
        .map(|step| match step {
            Step::Call { method } => json!({"do": "call", "method": method}),
            Step::Write { values } => json!({"do": "write", "values": values}),
            Step::Activity {
                summary,
                users,
                days,
            } => json!({"do": "activity", "summary": summary, "users": users, "days": days.unwrap_or(0)}),
            Step::ActivitiesDone => json!({"do": "done"}),
        })
        .collect();
    format!(
        "(async () => {{ {RPC}
  const model = {model}, id = {id}, page = {page}, steps = {steps};
  try {{
    let copy = await rpc(model, 'copy', [[id]]);
    copy = Array.isArray(copy) ? copy[0] : copy;
    for (const s of steps) {{
      if (s.do === 'call') await rpc(model, s.method, [[copy]]);
      else if (s.do === 'write') await rpc(model, 'write', [[copy], s.values]);
      else if (s.do === 'activity') {{
        let users = [null];
        if (s.users.length) users = await rpc('res.users', 'search', [[['name', 'in', s.users]]]);
        const due = new Date(Date.now() + s.days * 86400000).toISOString().slice(0, 10);
        for (const user of users) {{
          const kw = {{act_type_xmlid: 'mail.mail_activity_data_todo', summary: s.summary, date_deadline: due}};
          if (user) kw.user_id = user;
          await rpc(model, 'activity_schedule', [[copy]], kw);
        }}
      }} else if (s.do === 'done') {{
        const acts = await rpc('mail.activity', 'search', [[['res_model', '=', model], ['res_id', '=', copy]]]);
        if (acts.length) await rpc('mail.activity', 'action_feedback', [acts]);
      }}
    }}
    return JSON.stringify({{page: page.replace(/\\d+\\/?$/, String(copy)), copy}});
  }} catch (e) {{ return JSON.stringify({{error: String(e.message || e)}}); }}
}})()",
        model = json!(model),
        page = json!(page),
        steps = Value::Array(steps),
    )
}

/// Script removing the copy a scenario made (cancelling it first when needed).
/// Answers "[]" when done, else what could not be removed.
pub fn undo_script(model: &str, answer: &str) -> String {
    let copy = serde_json::from_str::<Value>(answer)
        .ok()
        .and_then(|v| v["copy"].as_u64());
    let Some(copy) = copy else {
        return "'[]'".into();
    };
    format!(
        "(async () => {{ {RPC}
  const model = {model}, copy = {copy};
  try {{ await rpc(model, 'unlink', [[copy]]); return '[]'; }} catch (e) {{}}
  try {{ await rpc(model, 'action_cancel', [[copy]]); await rpc(model, 'unlink', [[copy]]); return '[]'; }}
  catch (e) {{ return JSON.stringify(['kept a copy of ' + model + ' ' + copy + ' on the local demo database']); }}
}})()",
        model = json!(model),
    )
}

/// Script clicking the button (or link) whose text is `text`. Answers "ok" or "missing".
pub fn click_script(text: &str) -> String {
    format!(
        "(() => {{ const want = {t}.trim().toLowerCase();
  const all = [...document.querySelectorAll('button, a.btn, .o_statusbar_buttons button')];
  const b = all.find(e => e.offsetParent && e.innerText.trim().toLowerCase() === want);
  if (!b) return 'missing'; b.click(); return 'ok'; }})()",
        t = json!(text)
    )
}

const RPC: &str = "const rpc = async (model, method, args, kwargs = {}) => {
    const r = await fetch('/web/dataset/call_kw/' + model + '/' + method, {method: 'POST',
      headers: {'Content-Type': 'application/json'},
      body: JSON.stringify({jsonrpc: '2.0', method: 'call', params: {model, method, args, kwargs}})});
    const j = await r.json();
    if (j.error) throw new Error((j.error.data && j.error.data.message) || j.error.message);
    return j.result;
  };";

#[cfg(test)]
mod tests {
    use super::*;

    fn scenario(v: Value) -> Scenario {
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn the_record_id_ends_the_page() {
        assert_eq!(
            record_id("/odoo/action-stock.action_picking_tree_all/7"),
            Some(7)
        );
        assert_eq!(record_id("/odoo/sales/42/"), Some(42));
        assert_eq!(record_id("/odoo/sales"), None);
    }

    #[test]
    fn steps_run_on_a_copy_never_on_the_demo_record() {
        let s = scenario(json!({"steps": [
            {"do": "call", "method": "action_confirm"},
            {"do": "activity", "summary": "Grant Approval", "users": ["Marc Demo"], "days": 2},
            {"do": "activities_done"}]}));
        let js = script("stock.picking", "/odoo/inventory/receipts/7", &s);
        assert!(js.contains("'copy', [[id]]"));
        assert!(js.contains("rpc(model, s.method, [[copy]])"));
        assert!(js.contains("\"action_confirm\""));
        assert!(js.contains("Grant Approval"));
        assert!(!js.contains("[[id]], s.values"));
    }

    #[test]
    fn steps_need_a_model_a_record_and_real_method_names() {
        let s = scenario(json!({"steps": [{"do": "call", "method": "action_confirm"}]}));
        assert!(validate(None, "/odoo/sales/3", &s).is_err());
        assert!(validate(Some("sale.order"), "/odoo/sales", &s).is_err());
        assert!(validate(Some("sale.order"), "/odoo/sales/3", &s).is_ok());
        let bad = scenario(json!({"steps": [{"do": "call", "method": "_private(); alert(1)"}]}));
        assert!(validate(Some("sale.order"), "/odoo/sales/3", &bad).is_err());
        // Only a click needs nothing more.
        assert!(validate(
            None,
            "/odoo/sales",
            &scenario(json!({"click": "Mark as Todo"}))
        )
        .is_ok());
    }

    #[test]
    fn undo_removes_the_copy_it_made() {
        let js = undo_script("stock.picking", r#"{"page":"/odoo/x/9","copy":9}"#);
        assert!(js.contains("copy = 9") && js.contains("'unlink'"));
        assert_eq!(undo_script("stock.picking", r#"{"error":"x"}"#), "'[]'");
    }
}
