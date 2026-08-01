//! Cross-implementation fixture for every TSQ1 v1 feature.

use tsq1::{
    AbsoluteUnit, Event, EventKind, Marker, OscEvent, OscFormat, Sequence, SmpteFps, SmpteTiming,
    SyncAnchor, SysexEvent, TempoEntry, TimeDomain, Track, UnknownChunk,
};

fn full_featured_sequence() -> Sequence {
    Sequence {
        ppq: 960,
        absolute_unit: AbsoluteUnit::Nanoseconds,
        flags: 0x4003,
        tracks: vec![Track {
            events: vec![
                Event {
                    delta: 0,
                    domain: TimeDomain::Musical,
                    kind: EventKind::Midi([0x90, 60, 100]),
                },
                Event {
                    delta: 120,
                    domain: TimeDomain::Absolute,
                    kind: EventKind::Osc(OscEvent {
                        format: OscFormat::Raw,
                        data: b"/go\0,\0\0\0".to_vec(),
                    }),
                },
                Event {
                    delta: 240,
                    domain: TimeDomain::Musical,
                    kind: EventKind::Midi([0xC1, 8, 0]),
                },
                Event {
                    delta: 30,
                    domain: TimeDomain::Absolute,
                    kind: EventKind::Osc(OscEvent {
                        format: OscFormat::MessagePack,
                        data: vec![0x81, 0xA6, b'p', b'a', b'c', b'k', b'e', b't', 0x90],
                    }),
                },
                Event {
                    delta: 0,
                    domain: TimeDomain::Musical,
                    kind: EventKind::Osc(OscEvent {
                        format: OscFormat::Cbor,
                        data: vec![0xA1, 0x01, 0x80],
                    }),
                },
                Event {
                    delta: 480,
                    domain: TimeDomain::Musical,
                    kind: EventKind::Meta {
                        type_id: 0x06,
                        data: b"chorus".to_vec(),
                    },
                },
                Event {
                    delta: 0,
                    domain: TimeDomain::Absolute,
                    kind: EventKind::Sysex(SysexEvent {
                        status: 0xF7,
                        data: vec![0x01, 0x02, 0x03],
                    }),
                },
                Event {
                    delta: 1,
                    domain: TimeDomain::Absolute,
                    kind: EventKind::Custom {
                        type_id: 0x42,
                        data: vec![0xDE, 0xAD, 0xBE, 0xEF],
                    },
                },
            ],
        }],
        tempo_map: vec![
            TempoEntry {
                tick: 0,
                microseconds_per_quarter: 500_000,
            },
            TempoEntry {
                tick: 960,
                microseconds_per_quarter: 400_000,
            },
        ],
        sync_anchors: vec![
            SyncAnchor { tick: 0, time: 0 },
            SyncAnchor {
                tick: 960,
                time: 500_000_000,
            },
            SyncAnchor {
                tick: 1_920,
                time: 1_000_000_000,
            },
        ],
        markers: vec![
            Marker {
                domain: TimeDomain::Musical,
                position: 480,
                name: "Verse".into(),
                class: 0x00,
                color_rgba: None,
            },
            Marker {
                domain: TimeDomain::Absolute,
                position: 750_000_000,
                name: "Lights".into(),
                class: 0x20,
                color_rgba: Some(0xFF33_66CC),
            },
        ],
        smpte_timing: Some(SmpteTiming {
            fps: SmpteFps::Fps29Drop,
            subframes: 80,
        }),
        unknown_chunks: vec![UnknownChunk {
            id: *b"XTRA",
            data: vec![0xCA, 0xFE],
        }],
    }
}

fn decode_hex(input: &str) -> Vec<u8> {
    let compact: String = input
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    compact
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let text = core::str::from_utf8(pair).expect("fixture is ASCII");
            u8::from_str_radix(text, 16).expect("fixture contains hex")
        })
        .collect()
}

#[test]
fn shared_full_format_fixture_is_canonical() {
    let sequence = full_featured_sequence();
    let encoded = sequence.encode().expect("complete sequence encodes");
    let expected = decode_hex(include_str!(
        "../../../tests/fixtures/full-featured.tsq.hex"
    ));
    assert_eq!(
        encoded,
        expected,
        "replace the fixture with: {}",
        encoded
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    assert_eq!(
        Sequence::decode(&encoded).expect("fixture decodes"),
        sequence
    );
}
