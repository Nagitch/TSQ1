#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::borrow::Cow;
use alloc::vec::Vec;
use core::fmt;

use midly::num::u4;
use midly::{Fps, MetaMessage, MidiMessage, Smf, Timing, TrackEventKind};

/// Error type for TSQ1 conversions.
#[derive(Debug)]
pub enum Error {
    /// Underlying MIDI parsing error.
    Midi(midly::Error),
    /// Unsupported feature in the input file.
    Unsupported(&'static str),
    /// The resulting data exceeded format limits.
    DataOverflow(&'static str),
}

impl From<midly::Error> for Error {
    fn from(err: midly::Error) -> Self {
        Error::Midi(err)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Midi(e) => write!(f, "MIDI parse error: {e}"),
            Error::Unsupported(msg) => write!(f, "unsupported input: {msg}"),
            Error::DataOverflow(msg) => write!(f, "data overflow: {msg}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}

/// Convert SMF (Standard MIDI File) bytes into a TSQ1 binary buffer.
pub fn convert_midi_to_tsq_vec(midi_data: &[u8]) -> Result<Vec<u8>, Error> {
    let smf = Smf::parse(midi_data)?;
    convert_smf_to_tsq(&smf)
}

fn convert_smf_to_tsq(smf: &Smf<'_>) -> Result<Vec<u8>, Error> {
    let ppq = match smf.header.timing {
        Timing::Metrical(metrical) => metrical.as_int(),
        Timing::Timecode(_, _) => return Err(Error::Unsupported("SMPTE timecode timing")),
    };

    if smf.tracks.len() > u16::MAX as usize {
        return Err(Error::DataOverflow("too many tracks"));
    }

    let mut out = Vec::new();
    write_header(&mut out, ppq as u16, smf.tracks.len() as u16);

    for track in smf.tracks.iter() {
        let mut track_buf = Vec::new();
        for event in track {
            encode_event(event.delta.as_int() as u64, &event.kind, &mut track_buf)?;
        }
        if track_buf.len() > u32::MAX as usize {
            return Err(Error::DataOverflow("track chunk too large"));
        }
        out.extend_from_slice(b"TRK ");
        out.extend_from_slice(&(track_buf.len() as u32).to_le_bytes());
        out.extend_from_slice(&track_buf);
    }

    Ok(out)
}

fn write_header(out: &mut Vec<u8>, ppq: u16, track_count: u16) {
    out.extend_from_slice(b"TSQ1");
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&ppq.to_le_bytes());
    out.push(0); // AbsUnit = microseconds
    out.push(0); // Reserved
    out.extend_from_slice(&track_count.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // Flags
}

fn encode_event(delta: u64, kind: &TrackEventKind<'_>, out: &mut Vec<u8>) -> Result<(), Error> {
    const DOMAIN_MUSICAL: u8 = 0;

    match kind {
        TrackEventKind::Midi { channel, message } => {
            out.push(DOMAIN_MUSICAL | 0x01);
            write_vlq(delta, out);
            let status = midi_status_byte(*channel, message);
            let (data1, data2) = midi_message_bytes(message);
            out.push(status);
            out.push(data1);
            out.push(data2);
        }
        TrackEventKind::SysEx(data) | TrackEventKind::Escape(data) => {
            out.push(DOMAIN_MUSICAL | 0x03);
            write_vlq(delta, out);
            write_vlq(data.len() as u64, out);
            out.extend_from_slice(data);
        }
        TrackEventKind::Meta(meta) => {
            out.push(DOMAIN_MUSICAL | 0x02);
            write_vlq(delta, out);
            let (ty, payload) = meta_payload(meta);
            out.push(ty);
            write_vlq(payload.len() as u64, out);
            out.extend_from_slice(&payload);
        }
    }
    Ok(())
}

fn midi_status_byte(channel: u4, message: &MidiMessage) -> u8 {
    let nibble = match message {
        MidiMessage::NoteOff { .. } => 0x8,
        MidiMessage::NoteOn { .. } => 0x9,
        MidiMessage::Aftertouch { .. } => 0xA,
        MidiMessage::Controller { .. } => 0xB,
        MidiMessage::ProgramChange { .. } => 0xC,
        MidiMessage::ChannelAftertouch { .. } => 0xD,
        MidiMessage::PitchBend { .. } => 0xE,
    };
    (nibble << 4) | channel.as_int()
}

fn midi_message_bytes(message: &MidiMessage) -> (u8, u8) {
    match message {
        MidiMessage::NoteOff { key, vel }
        | MidiMessage::NoteOn { key, vel }
        | MidiMessage::Aftertouch { key, vel } => (key.as_int(), vel.as_int()),
        MidiMessage::Controller { controller, value } => (controller.as_int(), value.as_int()),
        MidiMessage::ProgramChange { program } => (program.as_int(), 0),
        MidiMessage::ChannelAftertouch { vel } => (vel.as_int(), 0),
        MidiMessage::PitchBend { bend } => {
            let raw = bend.0.as_int();
            ((raw & 0x7F) as u8, ((raw >> 7) & 0x7F) as u8)
        }
    }
}

fn meta_payload<'a>(meta: &MetaMessage<'a>) -> (u8, Cow<'a, [u8]>) {
    use MetaMessage::*;
    match meta {
        TrackNumber(Some(number)) => (0x00, Cow::Owned(number.to_be_bytes().to_vec())),
        TrackNumber(None) => (0x00, Cow::Borrowed(&[])),
        Text(data) => (0x01, Cow::Borrowed(data)),
        Copyright(data) => (0x02, Cow::Borrowed(data)),
        TrackName(data) => (0x03, Cow::Borrowed(data)),
        InstrumentName(data) => (0x04, Cow::Borrowed(data)),
        Lyric(data) => (0x05, Cow::Borrowed(data)),
        Marker(data) => (0x06, Cow::Borrowed(data)),
        CuePoint(data) => (0x07, Cow::Borrowed(data)),
        ProgramName(data) => (0x08, Cow::Borrowed(data)),
        DeviceName(data) => (0x09, Cow::Borrowed(data)),
        MidiChannel(channel) => (0x20, Cow::Owned(vec![channel.as_int()])),
        MidiPort(port) => (0x21, Cow::Owned(vec![port.as_int()])),
        EndOfTrack => (0x2F, Cow::Borrowed(&[])),
        Tempo(value) => {
            let raw = value.as_int();
            (0x51, Cow::Owned(raw.to_be_bytes()[1..].to_vec()))
        }
        SmpteOffset(smpte) => {
            let fps_code = match smpte.fps() {
                Fps::Fps24 => 0,
                Fps::Fps25 => 1,
                Fps::Fps29 => 2,
                Fps::Fps30 => 3,
            };
            let mut bytes = [0u8; 5];
            bytes[0] = smpte.hour() | (fps_code << 5);
            bytes[1] = smpte.minute();
            bytes[2] = smpte.second();
            bytes[3] = smpte.frame();
            bytes[4] = smpte.subframe();
            (0x54, Cow::Owned(bytes.to_vec()))
        }
        TimeSignature(a, b, c, d) => (0x58, Cow::Owned(vec![*a, *b, *c, *d])),
        KeySignature(sharps, is_minor) => (0x59, Cow::Owned(vec![*sharps as u8, *is_minor as u8])),
        SequencerSpecific(data) => (0x7F, Cow::Borrowed(data)),
        Unknown(ty, data) => (*ty, Cow::Borrowed(data)),
    }
}

fn write_vlq(mut value: u64, out: &mut Vec<u8>) {
    let mut buffer = [0u8; 10];
    let mut index = buffer.len();
    buffer[index - 1] = (value & 0x7F) as u8;
    index -= 1;
    value >>= 7;
    while value > 0 {
        buffer[index - 1] = ((value & 0x7F) as u8) | 0x80;
        index -= 1;
        value >>= 7;
    }
    out.extend_from_slice(&buffer[index..]);
}

/// FFI bindings for external consumers.
pub mod ffi {
    use super::*;
    use alloc::vec::Vec;
    use core::{mem, ptr, slice};

    /// Buffer returned by the FFI conversion helpers.
    #[repr(C)]
    pub struct Tsq1Buffer {
        pub ptr: *mut u8,
        pub len: usize,
        pub capacity: usize,
    }

    /// Status codes returned by the FFI API.
    #[repr(C)]
    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    pub enum Tsq1Status {
        Ok = 0,
        NullPointer = 1,
        ConversionError = 2,
    }

    /// Convert SMF bytes into TSQ1 format, allocating a new buffer for the result.
    ///
    /// The caller is responsible for freeing the resulting buffer with [`tsq1_buffer_free`].
    #[no_mangle]
    pub unsafe extern "C" fn tsq1_mid_to_tsq(
        midi_ptr: *const u8,
        midi_len: usize,
        out: *mut Tsq1Buffer,
    ) -> Tsq1Status {
        if midi_ptr.is_null() || out.is_null() {
            return Tsq1Status::NullPointer;
        }
        let midi = slice::from_raw_parts(midi_ptr, midi_len);
        match super::convert_midi_to_tsq_vec(midi) {
            Ok(mut data) => {
                let buffer = Tsq1Buffer {
                    ptr: data.as_mut_ptr(),
                    len: data.len(),
                    capacity: data.capacity(),
                };
                mem::forget(data);
                ptr::write(out, buffer);
                Tsq1Status::Ok
            }
            Err(_) => Tsq1Status::ConversionError,
        }
    }

    /// Release a buffer produced by [`tsq1_mid_to_tsq`].
    #[no_mangle]
    pub unsafe extern "C" fn tsq1_buffer_free(buf: Tsq1Buffer) {
        if buf.ptr.is_null() {
            return;
        }
        let _ = Vec::from_raw_parts(buf.ptr, buf.len, buf.capacity);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vlq_roundtrip_examples() {
        let mut data = Vec::new();
        write_vlq(0, &mut data);
        assert_eq!(data, vec![0]);
        data.clear();
        write_vlq(0x7F, &mut data);
        assert_eq!(data, vec![0x7F]);
        data.clear();
        write_vlq(0x80, &mut data);
        assert_eq!(data, vec![0x81, 0x00]);
    }
}
