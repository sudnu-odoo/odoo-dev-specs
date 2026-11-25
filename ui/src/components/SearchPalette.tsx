import Modal from "./Modal";
import { useEffect, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { useSessionSearch } from "../app/useSessionSearch";
import type { SearchHit } from "../lib/bindings/SearchHit";
import type { ProjectInfo, SessionRef } from "../lib/types";
import { Icon } from "./Icon";
import { versionLabel } from "./HostingMenu";

export interface SearchPaletteProps {
  /** Every project, to find them by name. */
  projects: ProjectInfo[];
  onOpen: (ref: SessionRef) => void;
  /** Show a project in the sidebar (and open its latest session). */
  onOpenProject: (name: string) => void;
  onClose: () => void;
}

/** One line of the results: a project, or a session. */
type Item = { kind: "project"; project: ProjectInfo } | { kind: "session"; hit: SearchHit };

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
 * Ctrl+K: find a project by its name, a session by its title, or a session by what
 * was said in it, each in its own section. Arrows move, Enter opens, Esc closes.
 */
export default function SearchPalette({ projects, onOpen, onOpenProject, onClose }: SearchPaletteProps) {
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState(0);
  const { hits, loading } = useSessionSearch(query);
  const list = useRef<HTMLDivElement>(null);

  const terms = query.toLowerCase().split(/\s+/).filter(Boolean);
  const matches = (text: string) => terms.every((t) => text.toLowerCase().includes(t));
  const sections: { title: string; items: Item[] }[] = terms.length
    ? [
        {
          title: "Projects",
          items: projects
            .filter((p) => p.name && matches(p.name))
            .map((project) => ({ kind: "project" as const, project })),
        },
        {
          title: "Sessions",
          items: hits
            .filter((h) => matches(h.session.title))
            .map((hit) => ({ kind: "session" as const, hit })),
        },
        {
          title: "In conversations",
          items: hits
            .filter((h) => !matches(h.session.title))
            .map((hit) => ({ kind: "session" as const, hit })),
        },
      ]
    : [{ title: "Recent", items: hits.map((hit) => ({ kind: "session" as const, hit })) }];
  const items = sections.flatMap((sec) => sec.items);
  const current = Math.min(selected, Math.max(items.length - 1, 0));

  useEffect(() => {
    list.current?.querySelector('[aria-selected="true"]')?.scrollIntoView?.({ block: "nearest" });
  }, [current]);

  function choose(item: Item | undefined) {
    if (!item) return;
    if (item.kind === "project") onOpenProject(item.project.name);
    else onOpen({ project: item.hit.project, sessionId: item.hit.session.id });
  }

  function onKey(e: KeyboardEvent<HTMLInputElement>) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setSelected(Math.min(current + 1, items.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSelected(Math.max(current - 1, 0));
    } else if (e.key === "Enter") {
      e.preventDefault();
      choose(items[current]);
    } else if (e.key === "Escape") {
      e.preventDefault();
      onClose();
    }
  }

  let index = -1;
  return (
    <Modal label="Search" onClose={onClose} place="top" width="680px">
      <div className="palette">
        <div className="palette-input">
          <Icon name="search" />
          <input
            autoFocus
            role="combobox"
            aria-label="Search"
            aria-expanded="true"
            aria-controls="palette-results"
            placeholder="Search projects, sessions and conversations…"
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
          {!loading && terms.length > 0 && items.length === 0 && (
            <p className="palette-empty">Nothing matches “{query.trim()}”.</p>
          )}
          {sections
            .filter((sec) => sec.items.length)
            .map((sec) => (
              <div key={sec.title} role="group" aria-label={sec.title}>
                <div className="palette-group-title">
                  {sec.title}
                  {terms.length > 0 && <span className="palette-count">{sec.items.length}</span>}
                </div>
                {sec.items.map((item) => {
                  const i = ++index;
                  const common = {
                    role: "option",
                    "aria-selected": i === current,
                    className: "palette-hit",
                    onMouseEnter: () => setSelected(i),
                    onClick: () => choose(item),
                  };
                  if (item.kind === "project") {
                    const n = item.project.sessions.length;
                    return (
                      <div key={`p/${item.project.name}`} {...common}>
                        <div className="palette-hit-head">
                          <Icon name="folder" />
                          <span className="palette-hit-title">{highlight(item.project.name, query)}</span>
                          <small>
                            {n} session{n === 1 ? "" : "s"}
                          </small>
                        </div>
                      </div>
                    );
                  }
                  const { hit } = item;
                  return (
                    <div key={`s/${hit.session.id}`} {...common}>
                      <div className="palette-hit-head">
                        <Icon name="chat" />
                        <span className="palette-hit-title">{highlight(hit.session.title, query)}</span>
                        <small>{[hit.project || "No project", where(hit)].filter(Boolean).join(" · ")}</small>
                      </div>
                      {hit.snippets.map((s, k) => (
                        <div key={k} className="palette-hit-snippet">
                          {highlight(s, query)}
                        </div>
                      ))}
                    </div>
                  );
                })}
              </div>
            ))}
        </div>
        <div className="palette-foot">
          <span>↑↓ move</span>
          <span>Enter open</span>
          <span>Esc close</span>
        </div>
      </div>
    </Modal>
  );
}
