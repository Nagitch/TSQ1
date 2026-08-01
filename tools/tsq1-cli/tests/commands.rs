//! End-to-end command tests for inspection and validation.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn fixture_file(name: &str, bytes: &[u8]) -> PathBuf {
    let path = std::env::temp_dir().join(format!("tsq1-cli-{}-{name}.tsq", std::process::id()));
    fs::write(&path, bytes).expect("write temporary fixture");
    path
}

fn canonical_fixture() -> Vec<u8> {
    let compact: String = include_str!("../../../tests/fixtures/full-featured.tsq.hex")
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    compact
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            u8::from_str_radix(core::str::from_utf8(pair).expect("ASCII fixture"), 16)
                .expect("hex fixture")
        })
        .collect()
}

#[test]
fn validate_reports_document_summary() {
    let path = fixture_file("valid", &canonical_fixture());
    let output = Command::new(env!("CARGO_BIN_EXE_tsq1-cli"))
        .args(["validate", path.to_str().expect("UTF-8 path")])
        .output()
        .expect("run validate");
    fs::remove_file(path).expect("remove temporary fixture");
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("1 track(s), 8 event(s), 2 marker(s)"));
}

#[test]
fn inspect_emits_complete_json() {
    let path = fixture_file("inspect", &canonical_fixture());
    let output = Command::new(env!("CARGO_BIN_EXE_tsq1-cli"))
        .args(["inspect", path.to_str().expect("UTF-8 path"), "--compact"])
        .output()
        .expect("run inspect");
    fs::remove_file(path).expect("remove temporary fixture");
    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    assert_eq!(
        value["tracks"][0]["events"].as_array().map(Vec::len),
        Some(8)
    );
    assert_eq!(value["smpte_timing"]["fps"], "fps29Drop");
}

#[test]
fn inspect_emits_large_u64_values_as_decimal_strings() {
    const LARGE: u64 = 9_007_199_254_740_993;

    let mut sequence = tsq1::Sequence::new(480);
    sequence.tracks.push(tsq1::Track {
        events: vec![tsq1::Event {
            delta: LARGE,
            domain: tsq1::TimeDomain::Musical,
            kind: tsq1::EventKind::Meta {
                type_id: 0x2F,
                data: Vec::new(),
            },
        }],
    });
    sequence.tempo_map.push(tsq1::TempoEntry {
        tick: LARGE,
        microseconds_per_quarter: 500_000,
    });
    sequence.sync_anchors.push(tsq1::SyncAnchor {
        tick: LARGE,
        time: LARGE,
    });
    sequence.markers.push(tsq1::Marker {
        domain: tsq1::TimeDomain::Absolute,
        position: LARGE,
        name: "large".into(),
        class: 0,
        color_rgba: None,
    });

    let path = fixture_file("large-u64", &sequence.encode().expect("encode fixture"));
    let output = Command::new(env!("CARGO_BIN_EXE_tsq1-cli"))
        .args(["inspect", path.to_str().expect("UTF-8 path"), "--compact"])
        .output()
        .expect("run inspect");
    fs::remove_file(path).expect("remove temporary fixture");
    assert!(output.status.success());

    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    let expected = LARGE.to_string();
    assert_eq!(value["tracks"][0]["events"][0]["delta"], expected);
    assert_eq!(value["tempo_map"][0]["tick"], expected);
    assert_eq!(value["sync_anchors"][0]["tick"], expected);
    assert_eq!(value["sync_anchors"][0]["time"], expected);
    assert_eq!(value["markers"][0]["position"], expected);

    let roundtrip: tsq1::Sequence = serde_json::from_value(value).expect("deserialize model");
    assert_eq!(roundtrip, sequence);
}

#[test]
fn validate_reports_malformed_byte_offset() {
    let path = fixture_file("invalid", b"TSQ1");
    let output = Command::new(env!("CARGO_BIN_EXE_tsq1-cli"))
        .args(["validate", path.to_str().expect("UTF-8 path")])
        .output()
        .expect("run validate");
    fs::remove_file(path).expect("remove temporary fixture");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("byte 0"));
}
