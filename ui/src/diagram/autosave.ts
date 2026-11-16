/**
 * Decides when the canvas is written to disk. Saves are debounced, skipped when
 * the canvas matches what is on disk, and held while the assistant edits the
 * same file (otherwise an older canvas could overwrite its newer work).
 */
export class Autosave {
  private saved: string;
  private pending: { fingerprint: string; serialize: () => string } | null = null;
  private timer: ReturnType<typeof setTimeout> | null = null;
  private held = false;

  constructor(
    private readonly save: (scene: string) => void,
    private readonly delayMs: number,
    savedFingerprint: string,
  ) {
    this.saved = savedFingerprint;
  }

  /** The canvas changed; `serialize` produces the file content if a save is due. */
  change(fingerprint: string, serialize: () => string): void {
    this.cancelTimer();
    if (fingerprint === this.saved) {
      // Back to what the disk holds: nothing (left) to save.
      this.pending = null;
      return;
    }
    this.pending = { fingerprint, serialize };
    this.schedule();
  }

  /** The disk now holds this content (it was just loaded or merged from there). */
  markSaved(fingerprint: string): void {
    this.cancelTimer();
    this.pending = null;
    this.saved = fingerprint;
  }

  /** Hold saves (assistant working) or release them. */
  hold(on: boolean): void {
    this.held = on;
    if (on) this.cancelTimer();
    else this.schedule();
  }

  /** Save a pending change now (leaving the session), unless saves are held. */
  flush(): void {
    this.cancelTimer();
    if (!this.held) this.write();
  }

  private schedule(): void {
    if (this.held || !this.pending || this.timer) return;
    this.timer = setTimeout(() => {
      this.timer = null;
      this.write();
    }, this.delayMs);
  }

  private write(): void {
    if (!this.pending) return;
    const { fingerprint, serialize } = this.pending;
    this.pending = null;
    this.saved = fingerprint;
    this.save(serialize());
  }

  private cancelTimer(): void {
    if (this.timer) clearTimeout(this.timer);
    this.timer = null;
  }
}
