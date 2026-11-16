export const DEFAULT_TITLE = "New spec";
const MAX = 60;

/** Session title derived from the first message, or null when there is no text. */
export function titleFromMessage(text: string): string | null {
  const firstLine = text.trim().split("\n")[0].replace(/\s+/g, " ").trim();
  if (!firstLine) return null;
  if (firstLine.length <= MAX) return firstLine;
  const cut = firstLine.slice(0, MAX - 1);
  const atWord = cut.lastIndexOf(" ");
  return (atWord > MAX / 2 ? cut.slice(0, atWord) : cut).trimEnd() + "…";
}
