type Versioned = { id: string; version: number };

/**
 * Combine the canvas (local) with the diagram on disk (incoming, e.g. after the
 * assistant edited it). Per element the higher version wins; on a tie the disk wins.
 * Order: the disk's order, then elements that only exist on the canvas.
 */
export function mergeElements<T extends Versioned>(local: readonly T[], incoming: readonly T[]): T[] {
  const localById = new Map(local.map((e) => [e.id, e]));
  const merged = incoming.map((disk) => {
    const mine = localById.get(disk.id);
    return mine && mine.version > disk.version ? mine : disk;
  });
  const onDisk = new Set(incoming.map((e) => e.id));
  return [...merged, ...local.filter((e) => !onDisk.has(e.id))];
}
