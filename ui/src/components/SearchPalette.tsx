import { useEffect, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { useSessionSearch } from "../app/useSessionSearch";
import type { SearchHit } from "../lib/bindings/SearchHit";
import type { SessionRef } from "../lib/types";
import { versionLabel } from "./HostingMenu";

export interface SearchPaletteProps {
  onOpen: (ref: SessionRef) => void;
  onClose: () => void;
}

/** "Online · 19.2" for a session whose client hosting and version are chosen. */
function where(hit: SearchHit): string {
  const s = hit.session;
  const hosting = s.hosting === "online" ? "Online" : s.hosting === "sh" ? "Odoo.sh" : null;
  return [hosting, s.version ? versionLabel(s.version) : null].filter(Boolean).join(" · ");
}

/** The words of the search in bold. */
function highlight(text: string, query: string): ReactNode {
  const terms = query.split(/\s+/).filter(Boolean);
  if (!terms.length) return text;
  const escaped = terms.map((t) => t.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"));
  return text
    .split(new RegExp(`(${escaped.join("|")})`, "gi"))
    .map((part, i) => (i % 2 ? <mark key={i}>{part}</mark> : part));
}

/**
 * Ctrl+K: find a session by its title or by what was said in it. Results are grouped
 * by client, with the passage that matched; arrows move, Enter opens, Esc closes.
 */
export default function SearchPalette({ onOpen, onClose }: SearchPaletteProps) {
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState(0);
  const { hits, loading } = useSessionSearch(query);
  const list = useRef<HTMLDivElement>(null);
  const current = Math.min(selected, Math.max(hits.length - 1, 0));

  useEffect(() => {
    list.current?.querySelector('[aria-selected="true"]')?.scrollIntoView?.({ block: "nearest" });
  }, [current]);

  const open = (hit: SearchHit) => onOpen({ project: hit.project, sessionId: hit.session.id });
  function onKey(e: KeyboardEvent<HTMLInputElement>) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setSelected(Math.min(current + 1, hits.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSelected(Math.max(current - 1, 0));
    } else if (e.key === "Enter" && hits[current]) {
      e.preventDefault();
      open(hits[current]);
    } else if (e.key === "Escape") {
      e.preventDefault();
      onClose();
    }
  }

  // Groups in the order their first result comes, keeping each result's place.
  const groups: { project: string; items: { hit: SearchHit; index: number }[] }[] = [];
  hits.forEach((hit, index) => {
    let group = groups.find((g) => g.project === hit.project);
    if (!group) groups.push((group = { project: hit.project, items: [] }));
    group.items.push({ hit, index });
  });

  return (
    <div className="palette-overlay" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="palette" role="dialog" aria-label="Search sessions">
        <div className="palette-input">
          <svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true">
            <circle cx="7" cy="7" r="4.5" fill="none" stroke="currentColor" strokeWidth="1.4" />
            <path d="m10.5 10.5 3 3" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
          </svg>
          <input
            autoFocus
            role="combobox"
            aria-label="Search sessions"
            aria-expanded="true"
            aria-controls="palette-results"
            placeholder="Search sessions and what was said in them…"
            value={query}
            spellCheck={false}
            onChange={(e) => {
              setQuery(e.target.value);
              setSelected(0);
            }}
            onKeyDown={onKey}
          />
          {loading && <span className="spinner spinner-small" aria-hidden="true" />}
          <kbd>Esc</kbd>
        </div>
        <div className="palette-results" id="palette-results" role="listbox" ref={list}>
          {!query.trim() && hits.length > 0 && <div className="palette-group-title">Recent</div>}
          {!loading && query.trim() && hits.length === 0 && (
            <p className="palette-empty">No session mentions “{query.trim()}”.</p>
          )}
          {groups.map((g) => (
            <div key={g.project} role="group" aria-label={g.project || "No project"}>
              {query.trim() && (
                <div className="palette-group-title">
                  {g.project || "No project"} · {g.items.length} session{g.items.length > 1 ? "s" : ""}
                </div>
              )}
              {g.items.map(({ hit, index }) => (
                <div
                  key={`${hit.project}/${hit.session.id}`}
                  role="option"
                  aria-selected={index === current}
                  className="palette-hit"
                  onMouseEnter={() => setSelected(index)}
                  onClick={() => open(hit)}
                >
                  <div className="palette-hit-head">
                    <span className="palette-hit-title">{highlight(hit.session.title, query)}</span>
                    <small>
                      {[!query.trim() ? hit.project : null, where(hit)].filter(Boolean).join(" · ")}
                    </small>
                  </div>
                  {hit.snippets.map((s, i) => (
                    <div key={i} className="palette-hit-snippet">
                      {highlight(s, query)}
                    </div>
                  ))}
                </div>
              ))}
            </div>
          ))}
        </div>
        <div className="palette-foot">
          <span>↑↓ move</span>
          <span>Enter open</span>
          <span>Esc close</span>
        </div>
      </div>
    </div>
  );
}
