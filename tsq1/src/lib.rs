#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::borrow::Cow;
use alloc::vec::Vec;
use core::convert::TryFrom;
use core::fmt;

use midly::num::{u14, u15, u24, u28, u4, u7};
use midly::{
    Format, Fps, Header, MetaMessage, MidiMessage, PitchBend, Smf, SmpteTime, Timing, TrackEvent,
    TrackEventKind,
};

/// Error type for TSQ1 conversions.
#[derive(Debug)]
pub enum Error {
    /// Underlying MIDI parsing error.
    Midi(midly::Error),
    /// Unsupported feature in the input file.
    Unsupported(&'static str),
    /// The resulting data exceeded format limits.
    DataOverflow(&'static str),
    /// Invalid or malformed TSQ input data.
    Invalid(&'static str),
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
            Error::Invalid(msg) => write!(f, "invalid input: {msg}"),
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

/// Convert TSQ1 bytes into a Standard MIDI File binary buffer.
pub fn convert_tsq_to_midi_vec(tsq_data: &[u8]) -> Result<Vec<u8>, Error> {
    let smf = convert_tsq_to_smf(tsq_data)?;
    let mut out = Vec::new();
    smf.write(&mut out)
        .map_err(|_| Error::Invalid("failed to encode SMF"))?;
    Ok(out)
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

fn convert_tsq_to_smf<'a>(tsq_data: &'a [u8]) -> Result<Smf<'a>, Error> {
    const HEADER_SIZE: usize = 14;
    if tsq_data.len() < HEADER_SIZE {
        return Err(Error::Invalid("TSQ header truncated"));
    }

    if &tsq_data[..4] != b"TSQ1" {
        return Err(Error::Invalid("TSQ magic missing"));
    }

    let version = u16::from_le_bytes([tsq_data[4], tsq_data[5]]);
    if version != 1 {
        return Err(Error::Unsupported("unsupported TSQ version"));
    }

    let ppq = u16::from_le_bytes([tsq_data[6], tsq_data[7]]);
    let abs_unit = tsq_data[8];
    if abs_unit != 0 {
        return Err(Error::Unsupported("absolute timing domain not supported"));
    }

    let track_count = u16::from_le_bytes([tsq_data[10], tsq_data[11]]);
    let _flags = u16::from_le_bytes([tsq_data[12], tsq_data[13]]);

    let timing =
        u15::try_from(ppq).ok_or(Error::Unsupported("PPQ exceeds SMF metrical timing range"))?;
    let format = if track_count <= 1 {
        Format::SingleTrack
    } else {
        Format::Parallel
    };

    let mut cursor = &tsq_data[HEADER_SIZE..];
    let mut tracks: Vec<Vec<TrackEvent<'a>>> = Vec::new();

    while !cursor.is_empty() {
        if cursor.len() < 8 {
            return Err(Error::Invalid("TSQ chunk header truncated"));
        }
        let id = &cursor[..4];
        let len = u32::from_le_bytes([cursor[4], cursor[5], cursor[6], cursor[7]]) as usize;
        cursor = &cursor[8..];
        if cursor.len() < len {
            return Err(Error::Invalid("TSQ chunk length exceeds remaining data"));
        }
        let chunk_data = &cursor[..len];
        cursor = &cursor[len..];

        if id == b"TRK " {
            let events = parse_track(chunk_data)?;
            tracks.push(events);
        }
    }

    if tracks.len() != track_count as usize {
        return Err(Error::Invalid("track count mismatch"));
    }

    Ok(Smf {
        header: Header::new(format, Timing::Metrical(timing)),
        tracks,
    })
}

fn parse_track<'a>(mut data: &'a [u8]) -> Result<Vec<TrackEvent<'a>>, Error> {
    let mut events = Vec::new();
    while !data.is_empty() {
        let header = read_u8(&mut data)?;
        let domain = header >> 7;
        if domain != 0 {
            return Err(Error::Unsupported(
                "absolute domain events are not supported",
            ));
        }
        let kind = header & 0x7F;
        let delta = read_vlq(&mut data)?;
        let delta_u32 =
            u32::try_from(delta).map_err(|_| Error::DataOverflow("delta exceeds u32"))?;
        let delta =
            u28::try_from(delta_u32).ok_or(Error::DataOverflow("delta exceeds MIDI limits"))?;

        let event_kind = match kind {
            0x01 => parse_midi_event(&mut data)?,
            0x02 => parse_meta_event(&mut data)?,
            0x03 => parse_sysex_event(&mut data)?,
            0x7E => return Err(Error::Unsupported("custom events are not supported")),
            _ => return Err(Error::Unsupported("unknown musical event type")),
        };

        events.push(TrackEvent {
            delta,
            kind: event_kind,
        });
    }
    Ok(events)
}

fn parse_midi_event<'a>(data: &mut &'a [u8]) -> Result<TrackEventKind<'a>, Error> {
    let status = read_u8(data)?;
    if !(0x80..=0xEF).contains(&status) {
        return Err(Error::Invalid("invalid MIDI status byte"));
    }
    let data1 = read_u8(data)?;
    let data2 = read_u8(data)?;

    let channel = u4::try_from(status & 0x0F).ok_or(Error::Invalid("invalid MIDI channel"))?;
    let message = match status >> 4 {
        0x8 => MidiMessage::NoteOff {
            key: u7::try_from(data1).ok_or(Error::Invalid("note key out of range"))?,
            vel: u7::try_from(data2).ok_or(Error::Invalid("velocity out of range"))?,
        },
        0x9 => MidiMessage::NoteOn {
            key: u7::try_from(data1).ok_or(Error::Invalid("note key out of range"))?,
            vel: u7::try_from(data2).ok_or(Error::Invalid("velocity out of range"))?,
        },
        0xA => MidiMessage::Aftertouch {
            key: u7::try_from(data1).ok_or(Error::Invalid("note key out of range"))?,
            vel: u7::try_from(data2).ok_or(Error::Invalid("velocity out of range"))?,
        },
        0xB => MidiMessage::Controller {
            controller: u7::try_from(data1).ok_or(Error::Invalid("controller out of range"))?,
            value: u7::try_from(data2).ok_or(Error::Invalid("controller value out of range"))?,
        },
        0xC => MidiMessage::ProgramChange {
            program: u7::try_from(data1).ok_or(Error::Invalid("program out of range"))?,
        },
        0xD => MidiMessage::ChannelAftertouch {
            vel: u7::try_from(data1).ok_or(Error::Invalid("aftertouch velocity out of range"))?,
        },
        0xE => {
            let lsb = u7::try_from(data1).ok_or(Error::Invalid("pitch bend LSB out of range"))?;
            let msb = u7::try_from(data2).ok_or(Error::Invalid("pitch bend MSB out of range"))?;
            let raw = ((msb.as_int() as u16) << 7) | lsb.as_int() as u16;
            let bend = u14::try_from(raw).ok_or(Error::Invalid("pitch bend value out of range"))?;
            MidiMessage::PitchBend {
                bend: PitchBend(bend),
            }
        }
        _ => return Err(Error::Invalid("unsupported MIDI status")),
    };

    Ok(TrackEventKind::Midi { channel, message })
}

fn parse_meta_event<'a>(data: &mut &'a [u8]) -> Result<TrackEventKind<'a>, Error> {
    let ty = read_u8(data)?;
    let len = read_vlq(data)?;
    let len_usize =
        usize::try_from(len).map_err(|_| Error::DataOverflow("meta payload too large"))?;
    let payload = take_slice(data, len_usize)?;
    let meta = meta_from_payload(ty, payload)?;
    Ok(TrackEventKind::Meta(meta))
}

fn parse_sysex_event<'a>(data: &mut &'a [u8]) -> Result<TrackEventKind<'a>, Error> {
    let len = read_vlq(data)?;
    let len_usize =
        usize::try_from(len).map_err(|_| Error::DataOverflow("sysex payload too large"))?;
    let payload = take_slice(data, len_usize)?;
    Ok(TrackEventKind::SysEx(payload))
}

fn meta_from_payload<'a>(ty: u8, data: &'a [u8]) -> Result<MetaMessage<'a>, Error> {
    use MetaMessage::*;
    Ok(match ty {
        0x00 => match data.len() {
            0 => TrackNumber(None),
            2 => {
                let number = u16::from_be_bytes([data[0], data[1]]);
                TrackNumber(Some(number))
            }
            _ => return Err(Error::Invalid("invalid track number payload")),
        },
        0x01 => Text(data),
        0x02 => Copyright(data),
        0x03 => TrackName(data),
        0x04 => InstrumentName(data),
        0x05 => Lyric(data),
        0x06 => Marker(data),
        0x07 => CuePoint(data),
        0x08 => ProgramName(data),
        0x09 => DeviceName(data),
        0x20 => {
            let channel = *data
                .get(0)
                .ok_or(Error::Invalid("missing MIDI channel value"))?;
            let channel =
                u4::try_from(channel).ok_or(Error::Invalid("MIDI channel out of range"))?;
            MidiChannel(channel)
        }
        0x21 => {
            let port = *data
                .get(0)
                .ok_or(Error::Invalid("missing MIDI port value"))?;
            let port = u7::try_from(port).ok_or(Error::Invalid("MIDI port out of range"))?;
            MidiPort(port)
        }
        0x2F => {
            if !data.is_empty() {
                return Err(Error::Invalid("end of track meta should be empty"));
            }
            EndOfTrack
        }
        0x51 => {
            if data.len() != 3 {
                return Err(Error::Invalid("tempo meta must be 3 bytes"));
            }
            let value = ((data[0] as u32) << 16) | ((data[1] as u32) << 8) | data[2] as u32;
            let tempo = u24::try_from(value).ok_or(Error::Invalid("tempo out of range"))?;
            Tempo(tempo)
        }
        0x54 => {
            if data.len() != 5 {
                return Err(Error::Invalid("smpte offset meta must be 5 bytes"));
            }
            let fps_code = data[0] >> 5;
            let fps = match fps_code {
                0 => Fps::Fps24,
                1 => Fps::Fps25,
                2 => Fps::Fps29,
                3 => Fps::Fps30,
                _ => return Err(Error::Invalid("invalid SMPTE FPS code")),
            };
            let hour = data[0] & 0x1F;
            let minute = data[1];
            let second = data[2];
            let frame = data[3];
            let subframe = data[4];
            let smpte = SmpteTime::new(hour, minute, second, frame, subframe, fps)
                .ok_or(Error::Invalid("invalid SMPTE offset values"))?;
            SmpteOffset(smpte)
        }
        0x58 => {
            if data.len() != 4 {
                return Err(Error::Invalid("time signature meta must be 4 bytes"));
            }
            TimeSignature(data[0], data[1], data[2], data[3])
        }
        0x59 => {
            if data.len() != 2 {
                return Err(Error::Invalid("key signature meta must be 2 bytes"));
            }
            let sharps = data[0] as i8;
            let is_minor = data[1] != 0;
            KeySignature(sharps, is_minor)
        }
        0x7F => SequencerSpecific(data),
        _ => Unknown(ty, data),
    })
}

fn read_u8<'a>(data: &mut &'a [u8]) -> Result<u8, Error> {
    if data.is_empty() {
        return Err(Error::Invalid("unexpected end of track data"));
    }
    let byte = data[0];
    *data = &data[1..];
    Ok(byte)
}

fn take_slice<'a>(data: &mut &'a [u8], len: usize) -> Result<&'a [u8], Error> {
    if data.len() < len {
        return Err(Error::Invalid("payload exceeds remaining track data"));
    }
    let (prefix, rest) = data.split_at(len);
    *data = rest;
    Ok(prefix)
}

fn read_vlq<'a>(data: &mut &'a [u8]) -> Result<u64, Error> {
    let mut value = 0u64;
    let mut read = 0usize;
    loop {
        let byte = read_u8(data)?;
        value = (value << 7) | (byte & 0x7F) as u64;
        read += 1;
        if read > 10 {
            return Err(Error::Invalid("VLQ exceeds maximum length"));
        }
        if byte & 0x80 == 0 {
            break;
        }
    }
    Ok(value)
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
        ptr::write(
            out,
            Tsq1Buffer {
                ptr: ptr::null_mut(),
                len: 0,
                capacity: 0,
            },
        );
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
