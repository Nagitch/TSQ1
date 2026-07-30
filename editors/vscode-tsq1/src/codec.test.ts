import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

import { decodeSequence, encodeSequence, FormatError } from "./codec.js";
import { emptySequence } from "./model.js";

function fixtureBytes(): Uint8Array {
  const fixture = readFileSync(
    resolve(process.cwd(), "../../tests/fixtures/full-featured.tsq.hex"),
    "utf8",
  ).replaceAll(/\s/g, "");
  return Uint8Array.from(Buffer.from(fixture, "hex"));
}

void test("TypeScript codec reproduces the Rust canonical fixture", () => {
  const bytes = fixtureBytes();
  const model = decodeSequence(bytes);
  assert.deepEqual(encodeSequence(model), bytes);
  assert.deepEqual(decodeSequence(encodeSequence(model)), model);
  assert.deepEqual(
    new Set(model.tracks.flatMap((track) => track.events.map((event) => event.kind.kind))),
    new Set(["osc", "midi", "meta", "sysex", "custom"]),
  );
  assert.equal(model.tempo_map.length, 2);
  assert.equal(model.sync_anchors.length, 3);
  assert.equal(model.markers.length, 2);
  assert.equal(model.smpte_timing?.fps, "fps29Drop");
  assert.equal(model.unknown_chunks.length, 1);
});

void test("u64 values beyond JavaScript safe integers remain exact", () => {
  const model = emptySequence();
  model.markers.push({
    domain: "absolute",
    position: "9007199254740993",
    name: "large",
    class: 0,
    color_rgba: null,
  });
  const decoded = decodeSequence(encodeSequence(model));
  assert.equal(decoded.markers[0]?.position, "9007199254740993");
});

void test("malformed input reports a byte offset", () => {
  const truncated = fixtureBytes().subarray(0, 19);
  assert.throws(
    () => decodeSequence(truncated),
    (error) => error instanceof FormatError && /byte 14|byte 18|byte 19/.test(error.message),
  );
});

void test("semantic decode errors also identify a source byte offset", () => {
  const invalidRaw = fixtureBytes();
  invalidRaw[31] = 0;
  assert.throws(
    () => decodeSequence(invalidRaw),
    (error) => error instanceof FormatError && /byte 0/.test(error.message),
  );
});

void test("all event kinds can be created, encoded, and removed", () => {
  const model = emptySequence();
  model.tracks[0]!.events = [
    {
      delta: 0,
      domain: "musical",
      kind: { kind: "osc", value: { format: "raw", data: [47, 120, 0, 0, 44, 0, 0, 0] } },
    },
    { delta: 0, domain: "musical", kind: { kind: "midi", value: [0x90, 60, 100] } },
    {
      delta: 0,
      domain: "musical",
      kind: { kind: "meta", value: { type_id: 1, data: [65] } },
    },
    {
      delta: 0,
      domain: "musical",
      kind: { kind: "sysex", value: { status: 0xf0, data: [1] } },
    },
    {
      delta: 0,
      domain: "absolute",
      kind: { kind: "custom", value: { type_id: 2, data: [3] } },
    },
  ];
  const decoded = decodeSequence(encodeSequence(model));
  assert.equal(decoded.tracks[0]?.events.length, 5);
  decoded.tracks[0]!.events.splice(0, 5);
  assert.equal(decodeSequence(encodeSequence(decoded)).tracks[0]?.events.length, 0);
});
