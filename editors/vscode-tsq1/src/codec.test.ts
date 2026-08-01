import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

import { decodeSequence, encodeSequence, FormatError } from "./codec.js";
import { emptySequence, removeEventPreservingTimeline, type Track } from "./model.js";
import {
  assertDocumentRevision,
  nextDocumentRevision,
  StaleDocumentRevisionError,
} from "./revision.js";

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

void test("unknown chunk IDs preserve arbitrary four-byte values", () => {
  const model = emptySequence();
  model.unknown_chunks.push({ id: [0xff, 0x80, 0x00, 0x41], data: [0xde, 0xad] });
  const encoded = encodeSequence(model);
  const decoded = decodeSequence(encoded);
  assert.deepEqual(decoded.unknown_chunks, model.unknown_chunks);
  assert.deepEqual(encodeSequence(decoded), encoded);
});

void test("event removal carries delta to the next event in the same domain", () => {
  const track: Track = {
    events: [
      { delta: 10, domain: "musical", kind: { kind: "midi", value: [0x90, 60, 100] } },
      { delta: 4, domain: "absolute", kind: { kind: "custom", value: { type_id: 1, data: [] } } },
      { delta: 20, domain: "musical", kind: { kind: "midi", value: [0x80, 60, 0] } },
      {
        delta: "9007199254740993",
        domain: "absolute",
        kind: { kind: "custom", value: { type_id: 2, data: [] } },
      },
    ],
  };

  removeEventPreservingTimeline(track, 0);
  assert.deepEqual(
    track.events.map((event) => [event.domain, event.delta]),
    [
      ["absolute", 4],
      ["musical", 30],
      ["absolute", "9007199254740993"],
    ],
  );
  removeEventPreservingTimeline(track, 0);
  assert.equal(track.events[1]?.delta, "9007199254740997");
});

void test("event removal is transactional when the carried delta overflows u64", () => {
  const track: Track = {
    events: [
      { delta: "18446744073709551615", domain: "musical", kind: { kind: "midi", value: [0x90, 60, 100] } },
      { delta: 1, domain: "musical", kind: { kind: "midi", value: [0x80, 60, 0] } },
    ],
  };
  const before = structuredClone(track);

  assert.throws(() => removeEventPreservingTimeline(track, 0), /exceeds u64/);
  assert.deepEqual(track, before);
});

void test("stale editor revisions are rejected and revisions never move backward", () => {
  assert.doesNotThrow(() => assertDocumentRevision(4, 4));
  assert.throws(
    () => assertDocumentRevision(3, 4),
    (error) =>
      error instanceof StaleDocumentRevisionError &&
      error.expected === 3 &&
      error.actual === 4,
  );
  assert.throws(() => assertDocumentRevision(Number.NaN, 4), StaleDocumentRevisionError);
  assert.equal(nextDocumentRevision(4), 5);
  assert.throws(() => nextDocumentRevision(Number.MAX_SAFE_INTEGER), /cannot be advanced safely/);
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
