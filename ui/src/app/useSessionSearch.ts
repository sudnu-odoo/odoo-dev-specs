import { useEffect, useState } from "react";
import { api } from "../lib/api";
import type { SearchHit } from "../lib/bindings/SearchHit";

/**
 * Sessions matching what is typed, by title and by what was said in them; recent
 * sessions while nothing is typed. Waits a moment while typing, and drops answers
 * that arrive after a newer search.
 */
export function useSessionSearch(query: string) {
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [searched, setSearched] = useState<string | null>(null);
  useEffect(() => {
    let current = true;
    const t = setTimeout(
      () =>
        api.searchSessions(query).then(
          (found) => {
            if (!current) return;
            setHits(found);
            setSearched(query);
          },
          () => current && setSearched(query),
        ),
      query.trim() ? 120 : 0,
    );
    return () => {
      current = false;
      clearTimeout(t);
    };
  }, [query]);
  return { hits, loading: searched !== query };
}
