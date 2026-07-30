import { randomBytes } from "node:crypto";

import * as vscode from "vscode";

import { TsqDocument } from "./document.js";
import type { DocumentState, Sequence } from "./model.js";

interface WebviewMessage {
  type: "ready" | "apply";
  model?: Sequence;
}

export class TsqEditorProvider implements vscode.CustomEditorProvider<TsqDocument> {
  private readonly editEmitter =
    new vscode.EventEmitter<vscode.CustomDocumentEditEvent<TsqDocument>>();
  private readonly panels = new Map<TsqDocument, Set<vscode.WebviewPanel>>();

  readonly onDidChangeCustomDocument = this.editEmitter.event;

  static register(context: vscode.ExtensionContext): vscode.Disposable {
    const provider = new TsqEditorProvider();
    context.subscriptions.push(provider);
    return vscode.window.registerCustomEditorProvider("tsq1.sequenceEditor", provider, {
      supportsMultipleEditorsPerDocument: true,
      webviewOptions: { retainContextWhenHidden: true },
    });
  }

  async openCustomDocument(
    uri: vscode.Uri,
    openContext: vscode.CustomDocumentOpenContext,
  ): Promise<TsqDocument> {
    const document = await TsqDocument.open(uri, openContext.backupId);
    document.onDidEdit((event) => this.editEmitter.fire(event));
    document.onDidChangeContent((state) => this.broadcast(document, state));
    return document;
  }

  async resolveCustomEditor(
    document: TsqDocument,
    panel: vscode.WebviewPanel,
  ): Promise<void> {
    panel.webview.options = { enableScripts: true };
    const documentPanels = this.panels.get(document) ?? new Set<vscode.WebviewPanel>();
    documentPanels.add(panel);
    this.panels.set(document, documentPanels);
    panel.onDidDispose(() => {
      documentPanels.delete(panel);
      if (documentPanels.size === 0) {
        this.panels.delete(document);
      }
    });
    panel.webview.onDidReceiveMessage(async (message: WebviewMessage) => {
      if (message.type === "ready") {
        await panel.webview.postMessage({ type: "state", state: document.state });
        return;
      }
      if (message.type === "apply" && message.model !== undefined) {
        try {
          document.applyModel(message.model);
          await panel.webview.postMessage({ type: "status", message: "Changes applied" });
        } catch (error) {
          await panel.webview.postMessage({
            type: "diagnostic",
            message: error instanceof Error ? error.message : String(error),
          });
        }
      }
    });
    panel.webview.html = renderEditorHtml(panel.webview, document.state);
  }

  saveCustomDocument(document: TsqDocument): Promise<void> {
    return document.save();
  }

  saveCustomDocumentAs(document: TsqDocument, destination: vscode.Uri): Promise<void> {
    return document.saveAs(destination);
  }

  revertCustomDocument(document: TsqDocument): Promise<void> {
    return document.revert();
  }

  backupCustomDocument(
    document: TsqDocument,
    context: vscode.CustomDocumentBackupContext,
  ): Promise<vscode.CustomDocumentBackup> {
    return document.backup(context.destination);
  }

  dispose(): void {
    this.editEmitter.dispose();
    this.panels.clear();
  }

  private broadcast(document: TsqDocument, state: DocumentState): void {
    for (const panel of this.panels.get(document) ?? []) {
      void panel.webview.postMessage({ type: "state", state });
    }
  }
}

function renderEditorHtml(webview: vscode.Webview, state: DocumentState): string {
  const nonce = randomBytes(16).toString("base64");
  const initial = JSON.stringify(state).replaceAll("<", "\\u003c");
  return /* html */ `<!doctype html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <meta http-equiv="Content-Security-Policy"
        content="default-src 'none'; style-src ${webview.cspSource} 'unsafe-inline'; script-src 'nonce-${nonce}'">
  <title>TSQ1 Sequence Editor</title>
  <style>
    :root { color-scheme: light dark; }
    body { margin: 0; padding: 24px; color: var(--vscode-foreground); background: var(--vscode-editor-background); font-family: var(--vscode-font-family); }
    header { display: flex; align-items: flex-start; justify-content: space-between; gap: 24px; margin-bottom: 18px; }
    h1 { font-size: 22px; margin: 0 0 6px; }
    h2 { font-size: 15px; margin: 0 0 12px; }
    .muted { color: var(--vscode-descriptionForeground); }
    .cards { display: grid; grid-template-columns: repeat(auto-fit, minmax(130px, 1fr)); gap: 10px; margin-bottom: 18px; }
    .card, section { border: 1px solid var(--vscode-panel-border); border-radius: 6px; background: var(--vscode-sideBar-background); }
    .card { padding: 12px; }
    .card strong { display: block; font-size: 18px; margin-top: 4px; }
    section { padding: 14px; margin-bottom: 14px; }
    .toolbar { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; }
    button, select { border: 1px solid var(--vscode-button-border, transparent); border-radius: 3px; padding: 6px 10px; color: var(--vscode-button-foreground); background: var(--vscode-button-background); }
    button:hover { background: var(--vscode-button-hoverBackground); }
    button.secondary, select { color: var(--vscode-dropdown-foreground); background: var(--vscode-dropdown-background); border-color: var(--vscode-dropdown-border); }
    table { width: 100%; border-collapse: collapse; margin-top: 12px; }
    th, td { text-align: left; padding: 7px 8px; border-bottom: 1px solid var(--vscode-panel-border); }
    th { color: var(--vscode-descriptionForeground); font-weight: 600; }
    textarea { box-sizing: border-box; width: 100%; min-height: 430px; resize: vertical; padding: 12px; border: 1px solid var(--vscode-input-border); color: var(--vscode-input-foreground); background: var(--vscode-input-background); font-family: var(--vscode-editor-font-family); font-size: var(--vscode-editor-font-size); }
    .diagnostic { display: none; padding: 11px 12px; margin-bottom: 14px; border-left: 4px solid var(--vscode-errorForeground); color: var(--vscode-errorForeground); background: var(--vscode-inputValidation-errorBackground); }
    .status { min-height: 20px; color: var(--vscode-testing-iconPassed); }
    .empty { padding: 18px 8px; color: var(--vscode-descriptionForeground); }
    .danger { color: var(--vscode-errorForeground); }
    @media (max-width: 700px) { body { padding: 14px; } header { display: block; } }
  </style>
</head>
<body>
  <header>
    <div>
      <h1>TSQ1 Sequence Editor</h1>
      <div class="muted">Binary-safe structured editing with full JSON access</div>
    </div>
    <div id="status" class="status" aria-live="polite"></div>
  </header>
  <div id="diagnostic" class="diagnostic" role="alert"></div>
  <div id="cards" class="cards"></div>
  <section>
    <h2>Tracks and events</h2>
    <div class="toolbar">
      <button id="add-track" class="secondary">Add track</button>
      <select id="event-kind" aria-label="Event kind">
        <option value="osc">OSC</option>
        <option value="midi">MIDI</option>
        <option value="meta">Meta</option>
        <option value="sysex">SysEx</option>
        <option value="custom">Custom</option>
      </select>
      <button id="add-event">Add event to first track</button>
    </div>
    <div id="events"></div>
  </section>
  <section>
    <h2>Complete sequence model</h2>
    <p class="muted">Edit header fields, timing maps, sync anchors, markers, SMPTE timing, payload bytes, and unknown chunks. Apply validates and re-encodes the binary document.</p>
    <textarea id="model-json" spellcheck="false" aria-label="TSQ1 JSON model"></textarea>
    <div class="toolbar" style="margin-top: 10px">
      <button id="apply">Apply JSON</button>
      <button id="format" class="secondary">Format JSON</button>
    </div>
  </section>
  <script nonce="${nonce}">
    const vscode = acquireVsCodeApi();
    let state = ${initial};
    const cards = document.getElementById("cards");
    const events = document.getElementById("events");
    const json = document.getElementById("model-json");
    const diagnostic = document.getElementById("diagnostic");
    const status = document.getElementById("status");

    function showDiagnostic(message) {
      diagnostic.textContent = message || "";
      diagnostic.style.display = message ? "block" : "none";
    }

    function card(label, value) {
      const item = document.createElement("div");
      item.className = "card";
      const caption = document.createElement("span");
      caption.className = "muted";
      caption.textContent = label;
      const strong = document.createElement("strong");
      strong.textContent = String(value);
      item.append(caption, strong);
      cards.append(item);
    }

    function refresh(resetText) {
      cards.replaceChildren();
      events.replaceChildren();
      showDiagnostic(state.error);
      if (!state.model) {
        const empty = document.createElement("div");
        empty.className = "empty danger";
        empty.textContent = "The source bytes are preserved. Correct the file externally or paste a valid complete model after reopening a valid TSQ1 file.";
        events.append(empty);
        json.value = "";
        json.disabled = true;
        return;
      }
      json.disabled = false;
      const model = state.model;
      const eventCount = model.tracks.reduce((sum, track) => sum + track.events.length, 0);
      card("PPQ", model.ppq);
      card("Tracks", model.tracks.length);
      card("Events", eventCount);
      card("Tempo entries", model.tempo_map.length);
      card("Sync anchors", model.sync_anchors.length);
      card("Markers", model.markers.length);
      if (resetText) json.value = JSON.stringify(model, null, 2);

      model.tracks.forEach((track, trackIndex) => {
        const heading = document.createElement("h3");
        heading.textContent = "Track " + (trackIndex + 1) + " · " + track.events.length + " event(s)";
        events.append(heading);
        const table = document.createElement("table");
        const head = document.createElement("thead");
        head.innerHTML = "<tr><th>#</th><th>Domain</th><th>Delta</th><th>Kind</th><th></th></tr>";
        const body = document.createElement("tbody");
        track.events.forEach((event, eventIndex) => {
          const row = document.createElement("tr");
          for (const value of [eventIndex + 1, event.domain, event.delta, event.kind.kind]) {
            const cell = document.createElement("td");
            cell.textContent = String(value);
            row.append(cell);
          }
          const action = document.createElement("td");
          const remove = document.createElement("button");
          remove.className = "secondary";
          remove.textContent = "Remove";
          remove.addEventListener("click", () => {
            model.tracks[trackIndex].events.splice(eventIndex, 1);
            submitModel(model);
          });
          action.append(remove);
          row.append(action);
          body.append(row);
        });
        table.append(head, body);
        events.append(table);
      });
    }

    function submitModel(model) {
      showDiagnostic("");
      vscode.postMessage({ type: "apply", model });
    }

    document.getElementById("apply").addEventListener("click", () => {
      try {
        submitModel(JSON.parse(json.value));
      } catch (error) {
        showDiagnostic("JSON error: " + error.message);
      }
    });
    document.getElementById("format").addEventListener("click", () => {
      try {
        json.value = JSON.stringify(JSON.parse(json.value), null, 2);
        showDiagnostic("");
      } catch (error) {
        showDiagnostic("JSON error: " + error.message);
      }
    });
    document.getElementById("add-track").addEventListener("click", () => {
      if (!state.model) return;
      state.model.tracks.push({ events: [] });
      submitModel(state.model);
    });
    document.getElementById("add-event").addEventListener("click", () => {
      if (!state.model) return;
      if (state.model.tracks.length === 0) state.model.tracks.push({ events: [] });
      const kind = document.getElementById("event-kind").value;
      const templates = {
        osc: { delta: 0, domain: "musical", kind: { kind: "osc", value: { format: "raw", data: [47, 103, 111, 0, 44, 0, 0, 0] } } },
        midi: { delta: 0, domain: "musical", kind: { kind: "midi", value: [144, 60, 100] } },
        meta: { delta: 0, domain: "musical", kind: { kind: "meta", value: { type_id: 6, data: [] } } },
        sysex: { delta: 0, domain: "musical", kind: { kind: "sysex", value: { status: 240, data: [] } } },
        custom: { delta: 0, domain: "musical", kind: { kind: "custom", value: { type_id: 0, data: [] } } }
      };
      state.model.tracks[0].events.push(templates[kind]);
      submitModel(state.model);
    });
    window.addEventListener("message", (event) => {
      const message = event.data;
      if (message.type === "state") {
        state = message.state;
        refresh(true);
      } else if (message.type === "diagnostic") {
        showDiagnostic(message.message);
      } else if (message.type === "status") {
        status.textContent = message.message;
        setTimeout(() => { status.textContent = ""; }, 1800);
      }
    });
    refresh(true);
    vscode.postMessage({ type: "ready" });
  </script>
</body>
</html>`;
}
