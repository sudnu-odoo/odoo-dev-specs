/** Odoo versions as the app chooses among them: only those on this computer. */

const key = (v: string): [number, number] => {
  const [major, minor] = v
    .replace(/^saas-/, "")
    .split(".")
    .map(Number);
  return [major || 0, minor || 0];
};

/** Newest first: 19.2, 19.0, 18.4, 18.0... */
export function newestFirst(versions: string[]): string[] {
  return [...versions].sort((a, b) => {
    const [x, y] = [key(a), key(b)];
    return y[0] - x[0] || y[1] - x[1];
  });
}

/**
 * The version to start with: the one saved in Settings when it is on this computer,
 * otherwise the newest stable one here, otherwise the newest one here.
 */
export function preferredVersion(local: string[], saved?: string | null): string | null {
  if (saved && local.includes(saved)) return saved;
  const sorted = newestFirst(local);
  return sorted.find((v) => !v.startsWith("saas-")) ?? sorted[0] ?? null;
}
