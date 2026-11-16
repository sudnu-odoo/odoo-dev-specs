// Placeholder data: real projects/sessions come from ~/OdooSpecs in milestone 2.
const PROJECTS = [
  { name: "Acme Corp", sessions: ["Delivery window on SO", "Custom invoice layout"] },
  { name: "Globex", sessions: ["Helpdesk SLA flow"] },
];

export default function Sidebar() {
  return (
    <aside className="sidebar">
      <div className="sidebar-header">
        <span className="brand">Odoo Dev Specs</span>
        <button className="btn btn-primary btn-small" disabled title="Available in milestone 2">
          + New session
        </button>
      </div>
      <nav className="tree">
        {PROJECTS.map((p) => (
          <div key={p.name} className="tree-project">
            <div className="tree-project-name">{p.name}</div>
            {p.sessions.map((s, i) => (
              <div key={s} className={"tree-session" + (p.name === "Acme Corp" && i === 0 ? " active" : "")}>
                {s}
              </div>
            ))}
          </div>
        ))}
      </nav>
    </aside>
  );
}
