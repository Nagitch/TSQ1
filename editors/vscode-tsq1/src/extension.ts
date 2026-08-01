import type * as vscode from "vscode";

import { TsqEditorProvider } from "./editorProvider.js";

export function activate(context: vscode.ExtensionContext): void {
  context.subscriptions.push(TsqEditorProvider.register(context));
}

export function deactivate(): void {
  // VS Code disposes all registered providers through the extension context.
}
