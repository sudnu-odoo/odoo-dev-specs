import { useState, type FormEvent } from "react";
import Modal from "./Modal";

export interface NewProjectDialogProps {
  /** Names already used, refused before asking the app. */
  existing: string[];
  /** Rejects with a message for the person. */
  onCreate: (name: string) => Promise<void>;
  onCancel: () => void;
}

/** Name a new project: a group for a client's sessions. */
export default function NewProjectDialog({ existing, onCreate, onCancel }: NewProjectDialogProps) {
  const [name, setName] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  async function submit(e: FormEvent) {
    e.preventDefault();
    const typed = name.trim();
    if (!typed || saving) return;
    if (existing.some((n) => n.toLowerCase() === typed.toLowerCase())) {
      setError(`A project named "${typed}" already exists.`);
      return;
    }
    setSaving(true);
    try {
      await onCreate(typed);
    } catch (err) {
      setError(String(err));
      setSaving(false);
    }
  }

  return (
    <Modal label="New project" onClose={onCancel} width="380px">
      <form
        className="dialog dialog-small"
        onSubmit={submit}
        onKeyDown={(e) => e.key === "Escape" && onCancel()}
      >
        <h2>New project</h2>
        <input
          autoFocus
          className="dialog-input"
          aria-label="Project name"
          placeholder="Project name, e.g. the client"
          value={name}
          onChange={(e) => {
            setName(e.target.value);
            setError(null);
          }}
        />
        {error && (
          <div role="alert" className="dialog-error">
            {error}
          </div>
        )}
        <div className="dialog-actions">
          <button type="button" className="btn" onClick={onCancel}>
            Cancel
          </button>
          <button type="submit" className="btn btn-primary" disabled={saving || !name.trim()}>
            Create
          </button>
        </div>
      </form>
    </Modal>
  );
}
