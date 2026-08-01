//! Integration between TSQ1 OSC events and the `osc-ir` data model.

use std::error;
use std::fmt;

use osc_codec_msgpack::{try_from_msgpack, try_to_msgpack};
pub use osc_ir;
use osc_ir::IrValue;
use tsq1::{Event, EventKind, OscEvent, OscFormat, TimeDomain};

/// Error produced by OSC integration helpers.
#[derive(Debug)]
pub enum Error {
    /// The TSQ1 event is not an OSC event.
    NotOscEvent,
    /// The OSC event does not contain MessagePack IR.
    NotMessagePack,
    /// RAW OSC packet validation failed.
    InvalidRaw(&'static str),
    /// MessagePack encoding failed.
    Encode(String),
    /// MessagePack decoding failed.
    Decode(String),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotOscEvent => formatter.write_str("event is not an OSC event"),
            Self::NotMessagePack => formatter.write_str("OSC event is not MessagePack encoded"),
            Self::InvalidRaw(message) => write!(formatter, "invalid RAW OSC packet: {message}"),
            Self::Encode(message) => write!(formatter, "failed to encode OSC IR: {message}"),
            Self::Decode(message) => write!(formatter, "failed to decode OSC IR: {message}"),
        }
    }
}

impl error::Error for Error {}

/// Encode an `osc-ir` value into a TSQ1 MessagePack OSC event.
pub fn event_from_ir(domain: TimeDomain, delta: u64, value: &IrValue) -> Result<Event, Error> {
    let data = try_to_msgpack(value).map_err(|error| Error::Encode(error.to_string()))?;
    Ok(Event {
        delta,
        domain,
        kind: EventKind::Osc(OscEvent {
            format: OscFormat::MessagePack,
            data,
        }),
    })
}

/// Decode a TSQ1 MessagePack OSC event into an `osc-ir` value.
pub fn event_to_ir(event: &Event) -> Result<IrValue, Error> {
    let EventKind::Osc(osc) = &event.kind else {
        return Err(Error::NotOscEvent);
    };
    if osc.format != OscFormat::MessagePack {
        return Err(Error::NotMessagePack);
    }
    try_from_msgpack(&osc.data).map_err(|error| Error::Decode(error.to_string()))
}

/// Construct a validated byte-accurate RAW OSC TSQ1 event.
pub fn raw_event(domain: TimeDomain, delta: u64, data: Vec<u8>) -> Result<Event, Error> {
    if !matches!(data.first(), Some(b'/') | Some(b'#')) {
        return Err(Error::InvalidRaw("packet must start with '/' or '#'"));
    }
    if !data.len().is_multiple_of(4) {
        return Err(Error::InvalidRaw("packet must be four-byte aligned"));
    }
    Ok(Event {
        delta,
        domain,
        kind: EventKind::Osc(OscEvent {
            format: OscFormat::Raw,
            data,
        }),
    })
}

#[cfg(test)]
mod tests {
    use osc_ir::{IrBundle, IrTimetag};

    use super::*;

    #[test]
    fn nested_bundle_roundtrips() {
        let mut nested = IrBundle::new(IrTimetag::from_ntp(42));
        nested.add_message(IrValue::from("nested"));
        let mut root = IrBundle::immediate();
        root.add_message(IrValue::from(7));
        root.add_bundle(nested);
        let value = IrValue::Bundle(root);

        let event = event_from_ir(TimeDomain::Absolute, 125, &value).expect("encode");
        let decoded = event_to_ir(&event).expect("decode");
        assert_eq!(decoded, value);
    }

    #[test]
    fn raw_packet_validation_is_deterministic() {
        assert!(raw_event(TimeDomain::Musical, 0, vec![0, 1, 2, 3]).is_err());
        assert!(raw_event(TimeDomain::Musical, 0, b"/bad".to_vec()).is_ok());
        assert!(raw_event(TimeDomain::Musical, 0, b"/no\0\0".to_vec()).is_err());
        let bundle =
            raw_event(TimeDomain::Absolute, 4, b"#bundle\0".to_vec()).expect("aligned RAW bundle");
        let EventKind::Osc(osc) = bundle.kind else {
            panic!("expected OSC event");
        };
        assert_eq!(osc.data, b"#bundle\0");
    }
}
