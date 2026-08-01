export class StaleDocumentRevisionError extends Error {
  constructor(
    readonly expected: number,
    readonly actual: number,
  ) {
    super(`Editor revision ${expected} is stale; the current document revision is ${actual}`);
    this.name = "StaleDocumentRevisionError";
  }
}

export function assertDocumentRevision(expected: number, actual: number): void {
  if (!Number.isSafeInteger(expected) || expected < 0 || expected !== actual) {
    throw new StaleDocumentRevisionError(expected, actual);
  }
}

export function nextDocumentRevision(current: number): number {
  if (!Number.isSafeInteger(current) || current < 0 || current === Number.MAX_SAFE_INTEGER) {
    throw new RangeError("document revision cannot be advanced safely");
  }
  return current + 1;
}
