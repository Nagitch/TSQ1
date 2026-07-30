# TSQ1 Sequence Editor

This VS Code extension opens `.tsq` binary files as an editable sequence model.
It shows the header, tracks and events, tempo map, synchronization anchors,
markers, SMPTE source timing, and forward-compatible unknown chunks.

The event table provides templates for OSC, MIDI, meta, SysEx, and custom
events. The complete JSON model remains editable for precise changes to every
field. Apply operations participate in VS Code undo/redo, and normal save,
save-as, revert, and hot-exit backup flows are supported.

Invalid files remain open for diagnosis. Errors identify the byte offset at
which decoding failed and the original bytes are not overwritten.

## Development

```console
npm ci
npm run check
npm run lint
npm test
npm run vsix
```

The generated package is `dist/tsq1-editor.vsix`.
