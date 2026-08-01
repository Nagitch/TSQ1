import * as vscode from "vscode";

import { decodeSequence, encodeSequence } from "./codec.js";
import { emptySequence, type DocumentState, type Sequence } from "./model.js";
import { assertDocumentRevision, nextDocumentRevision } from "./revision.js";

interface Snapshot {
  bytes: Uint8Array;
  state: DocumentState;
}

export class TsqDocument implements vscode.CustomDocument {
  private current: Snapshot;
  private readonly changeEmitter = new vscode.EventEmitter<DocumentState>();
  private readonly editEmitter = new vscode.EventEmitter<vscode.CustomDocumentEditEvent<TsqDocument>>();

  readonly onDidChangeContent = this.changeEmitter.event;
  readonly onDidEdit = this.editEmitter.event;

  private constructor(
    readonly uri: vscode.Uri,
    snapshot: Snapshot,
  ) {
    this.current = snapshot;
  }

  static async open(uri: vscode.Uri, backupId?: string): Promise<TsqDocument> {
    if (uri.scheme === "untitled" && backupId === undefined) {
      const model = emptySequence();
      return new TsqDocument(uri, {
        bytes: encodeSequence(model),
        state: { model, error: null, revision: 0 },
      });
    }
    const source = backupId === undefined ? uri : vscode.Uri.parse(backupId);
    const bytes = await vscode.workspace.fs.readFile(source);
    return new TsqDocument(uri, snapshotFromBytes(bytes));
  }

  get state(): DocumentState {
    return cloneState(this.current.state);
  }

  get bytes(): Uint8Array {
    return this.current.bytes.slice();
  }

  applyModel(model: Sequence, expectedRevision: number): void {
    assertDocumentRevision(expectedRevision, this.current.state.revision);
    this.commitModel(model);
  }

  editModel(expectedRevision: number, edit: (model: Sequence) => void): void {
    assertDocumentRevision(expectedRevision, this.current.state.revision);
    const model = cloneState(this.current.state).model;
    if (model === null) {
      throw new Error("document has no editable sequence model");
    }
    edit(model);
    this.commitModel(model);
  }

  private commitModel(model: Sequence): void {
    const bytes = encodeSequence(model);
    const before = cloneSnapshot(this.current);
    const after: Snapshot = {
      bytes,
      state: {
        model: decodeSequence(bytes),
        error: null,
        revision: this.current.state.revision,
      },
    };
    this.restore(after);
    this.editEmitter.fire({
      document: this,
      label: "Edit TSQ1 sequence",
      undo: async () => {
        this.restore(before);
      },
      redo: async () => {
        this.restore(after);
      },
    });
  }

  async save(): Promise<void> {
    await vscode.workspace.fs.writeFile(this.uri, this.current.bytes);
  }

  async saveAs(destination: vscode.Uri): Promise<void> {
    await vscode.workspace.fs.writeFile(destination, this.current.bytes);
  }

  async revert(): Promise<void> {
    const bytes = await vscode.workspace.fs.readFile(this.uri);
    this.restore(snapshotFromBytes(bytes));
  }

  async backup(destination: vscode.Uri): Promise<vscode.CustomDocumentBackup> {
    await vscode.workspace.fs.writeFile(destination, this.current.bytes);
    return {
      id: destination.toString(),
      delete: async () => {
        try {
          await vscode.workspace.fs.delete(destination);
        } catch {
          // VS Code may already have removed an obsolete backup.
        }
      },
    };
  }

  dispose(): void {
    this.changeEmitter.dispose();
    this.editEmitter.dispose();
  }

  private restore(snapshot: Snapshot): void {
    const revision = nextDocumentRevision(this.current.state.revision);
    this.current = cloneSnapshot(snapshot);
    this.current.state.revision = revision;
    this.changeEmitter.fire(this.state);
  }
}

function snapshotFromBytes(bytes: Uint8Array): Snapshot {
  const owned = bytes.slice();
  try {
    return {
      bytes: owned,
      state: { model: decodeSequence(owned), error: null, revision: 0 },
    };
  } catch (error) {
    return {
      bytes: owned,
      state: {
        model: null,
        error: error instanceof Error ? error.message : String(error),
        revision: 0,
      },
    };
  }
}

function cloneSnapshot(snapshot: Snapshot): Snapshot {
  return {
    bytes: snapshot.bytes.slice(),
    state: cloneState(snapshot.state),
  };
}

function cloneState(state: DocumentState): DocumentState {
  return structuredClone(state);
}
