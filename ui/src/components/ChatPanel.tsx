export default function ChatPanel() {
  return (
    <section className="chat">
      <div className="chat-messages">
        <div className="chat-empty">
          <h2>Describe the feature to spec</h2>
          <p>
            The assistant reads the Odoo source, starts a fresh local database, takes screenshots and builds
            the diagram on the right.
          </p>
        </div>
      </div>
      <form className="chat-input" onSubmit={(e) => e.preventDefault()}>
        <textarea
          placeholder="e.g. Add a delivery window on sales orders, shown on the delivery slip…"
          rows={3}
          disabled
        />
        <button className="btn btn-primary" disabled title="AI providers connect in milestone 2">
          Send
        </button>
      </form>
    </section>
  );
}
