//! Complete TSQ1 sequence model and conversion primitives.
//!
//! This crate decodes, validates, edits, and canonically encodes the TSQ1
//! binary timeline format, and converts compatible events to and from Standard
//! MIDI Files (SMF). The default `std` feature also exposes standard error
//! integration and a C-compatible allocation API.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::borrow::Cow;
use alloc::vec;
use alloc::vec::Vec;
use core::convert::TryFrom;
use core::fmt;

use midly::num::{u14, u15, u24, u28, u4, u7};
use midly::{
    Format, Fps, Header, MetaMessage, MidiMessage, PitchBend, Smf, SmpteTime, Timing, TrackEvent,
    TrackEventKind,
};

mod model;

pub use model::{
    AbsoluteUnit, Event, EventKind, Marker, OscEvent, OscFormat, Sequence, SmpteFps, SmpteTiming,
    SyncAnchor, SysexEvent, TempoEntry, TimeDomain, Track, UnknownChunk,
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
    /// Invalid data at a byte offset in the TSQ1 input.
    InvalidAt {
        /// Zero-based byte offset.
        offset: usize,
        /// Description of the invalid data.
        message: &'static str,
    },
    /// A requested conversion has no valid timing mapping.
    TimeMapping(&'static str),
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
            Error::InvalidAt { offset, message } => {
                write!(f, "invalid input at byte {offset}: {message}")
            }
            Error::TimeMapping(msg) => write!(f, "time mapping error: {msg}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}

/// Convert SMF (Standard MIDI File) bytes into a TSQ1 binary buffer.
pub fn convert_midi_to_tsq_vec(midi_data: &[u8]) -> Result<Vec<u8>, Error> {
    let smf = Smf::parse(midi_data)?;
    sequence_from_smf(&smf)?.encode()
}

/// Convert TSQ1 bytes into a Standard MIDI File binary buffer.
pub fn convert_tsq_to_midi_vec(tsq_data: &[u8]) -> Result<Vec<u8>, Error> {
    let sequence = Sequence::decode(tsq_data)?;
    sequence_to_midi(&sequence)
}

fn sequence_from_smf(smf: &Smf<'_>) -> Result<Sequence, Error> {
    if smf.tracks.len() > u16::MAX as usize {
        return Err(Error::DataOverflow("too many tracks"));
    }
    let (ppq, domain, smpte_timing) = match smf.header.timing {
        Timing::Metrical(ppq) => (ppq.as_int(), TimeDomain::Musical, None),
        Timing::Timecode(fps, subframes) => {
            if subframes == 0 {
                return Err(Error::Invalid("SMPTE subframes must be greater than zero"));
            }
            (
                480,
                TimeDomain::Absolute,
                Some(SmpteTiming {
                    fps: smpte_fps_from_midly(fps),
                    subframes,
                }),
            )
        }
    };
    let mut sequence = Sequence::new(ppq);
    sequence.smpte_timing = smpte_timing;

    for source_track in &smf.tracks {
        let mut track = Track::default();
        let mut musical_position = 0u64;
        let mut timecode_remainder = 0u128;
        for source_event in source_track {
            let source_delta = u64::from(source_event.delta.as_int());
            let delta = match smpte_timing {
                Some(timing) => smpte_ticks_to_absolute_delta(
                    source_delta,
                    timing,
                    sequence.absolute_unit,
                    &mut timecode_remainder,
                )?,
                None => source_delta,
            };
            if domain == TimeDomain::Musical {
                musical_position = musical_position
                    .checked_add(delta)
                    .ok_or(Error::DataOverflow("musical position exceeds u64"))?;
                if let TrackEventKind::Meta(MetaMessage::Tempo(value)) = &source_event.kind {
                    sequence.tempo_map.push(TempoEntry {
                        tick: musical_position,
                        microseconds_per_quarter: value.as_int(),
                    });
                }
            }
            track.events.push(Event {
                delta,
                domain,
                kind: event_kind_from_midly(&source_event.kind),
            });
        }
        sequence.tracks.push(track);
    }
    sequence.tempo_map.sort_by_key(|entry| entry.tick);
    // The stable sort preserves source order, so replacing an equal-tick entry
    // keeps the tempo that is effective after all events at that position.
    let mut canonical_tempo_map: Vec<TempoEntry> = Vec::with_capacity(sequence.tempo_map.len());
    for entry in sequence.tempo_map.drain(..) {
        if let Some(previous) = canonical_tempo_map.last_mut() {
            if previous.tick == entry.tick {
                *previous = entry;
                continue;
            }
        }
        canonical_tempo_map.push(entry);
    }
    sequence.tempo_map = canonical_tempo_map;
    Ok(sequence)
}

fn event_kind_from_midly(kind: &TrackEventKind<'_>) -> EventKind {
    match kind {
        TrackEventKind::Midi { channel, message } => {
            let status = midi_status_byte(*channel, message);
            let (data1, data2) = midi_message_bytes(message);
            EventKind::Midi([status, data1, data2.unwrap_or(0)])
        }
        TrackEventKind::Meta(meta) => {
            let (type_id, data) = meta_payload(meta);
            EventKind::Meta {
                type_id,
                data: data.into_owned(),
            }
        }
        TrackEventKind::SysEx(data) => EventKind::Sysex(SysexEvent {
            status: 0xF0,
            data: data.to_vec(),
        }),
        TrackEventKind::Escape(data) => EventKind::Sysex(SysexEvent {
            status: 0xF7,
            data: data.to_vec(),
        }),
    }
}

fn sequence_to_midi(sequence: &Sequence) -> Result<Vec<u8>, Error> {
    let use_smpte = sequence.smpte_timing.is_some()
        && sequence
            .tracks
            .iter()
            .flat_map(|track| &track.events)
            .all(|event| event.domain == TimeDomain::Absolute);
    let timing = if use_smpte {
        let timing = sequence.smpte_timing.expect("checked above");
        Timing::Timecode(smpte_fps_to_midly(timing.fps), timing.subframes)
    } else {
        let ppq = u15::try_from(sequence.ppq)
            .ok_or(Error::Unsupported("PPQ exceeds SMF metrical timing range"))?;
        Timing::Metrical(ppq)
    };

    let reconcile_tempo_map = !use_smpte && !sequence.tempo_map.is_empty();
    let track_count = if reconcile_tempo_map {
        sequence.tracks.len().max(1)
    } else {
        sequence.tracks.len()
    };
    let mut tracks = Vec::with_capacity(track_count);
    for track_index in 0..track_count {
        let mut positioned_midi = Vec::new();
        if let Some(source_track) = sequence.tracks.get(track_index) {
            for (position, index, event) in position_events(sequence, source_track, use_smpte)? {
                if reconcile_tempo_map && is_tempo_event(&event.kind) {
                    continue;
                }
                let order = if is_end_of_track(&event.kind) { 2 } else { 0 };
                positioned_midi.push((position, order, index, midly_kind_from_event(&event.kind)?));
            }
        }
        if reconcile_tempo_map && track_index == 0 {
            for (index, entry) in sequence.tempo_map.iter().enumerate() {
                let tempo = u24::try_from(entry.microseconds_per_quarter)
                    .ok_or(Error::Invalid("tempo out of range"))?;
                positioned_midi.push((
                    entry.tick,
                    1,
                    index,
                    TrackEventKind::Meta(MetaMessage::Tempo(tempo)),
                ));
            }
            if sequence.tracks.is_empty() {
                positioned_midi.push((0, 2, 0, TrackEventKind::Meta(MetaMessage::EndOfTrack)));
            }
        }

        let terminal_position = positioned_midi
            .iter()
            .map(|(position, _, _, _)| *position)
            .max()
            .unwrap_or(0);
        for (position, _, _, kind) in &mut positioned_midi {
            if matches!(&*kind, TrackEventKind::Meta(MetaMessage::EndOfTrack)) {
                // SMF readers treat this marker as terminal, so it must follow
                // both merged-domain events and synthesized tempo changes.
                *position = terminal_position;
            }
        }
        positioned_midi.sort_by_key(|(position, order, index, _)| (*position, *order, *index));

        let mut previous = 0u64;
        let mut track = Vec::new();
        for (position, _, _, kind) in positioned_midi {
            let delta = position
                .checked_sub(previous)
                .ok_or(Error::Invalid("events are not ordered"))?;
            let delta_u32 =
                u32::try_from(delta).map_err(|_| Error::DataOverflow("MIDI delta exceeds u32"))?;
            let delta = u28::try_from(delta_u32)
                .ok_or(Error::DataOverflow("MIDI delta exceeds SMF limits"))?;
            track.push(TrackEvent { delta, kind });
            previous = position;
        }
        tracks.push(track);
    }

    let format = if tracks.len() <= 1 {
        Format::SingleTrack
    } else {
        Format::Parallel
    };
    let smf = Smf {
        header: Header::new(format, timing),
        tracks,
    };
    let mut out = Vec::new();
    smf.write(&mut out)
        .map_err(|_| Error::Invalid("failed to encode SMF"))?;
    Ok(out)
}

fn position_events<'a>(
    sequence: &Sequence,
    track: &'a Track,
    use_smpte: bool,
) -> Result<Vec<(u64, usize, &'a Event)>, Error> {
    let mut musical = 0u64;
    let mut absolute = 0u64;
    let mut result = Vec::with_capacity(track.events.len());
    for (index, event) in track.events.iter().enumerate() {
        let domain_position = match event.domain {
            TimeDomain::Musical => {
                musical = musical
                    .checked_add(event.delta)
                    .ok_or(Error::DataOverflow("musical position exceeds u64"))?;
                musical
            }
            TimeDomain::Absolute => {
                absolute = absolute
                    .checked_add(event.delta)
                    .ok_or(Error::DataOverflow("absolute position exceeds u64"))?;
                absolute
            }
        };
        let position = if use_smpte {
            absolute_to_smpte_ticks(
                domain_position,
                sequence.smpte_timing.expect("checked by caller"),
                sequence.absolute_unit,
            )?
        } else {
            match event.domain {
                TimeDomain::Musical => domain_position,
                TimeDomain::Absolute => sequence.absolute_to_tick(domain_position)?,
            }
        };
        result.push((position, index, event));
    }
    result.sort_by_key(|(position, index, _)| (*position, *index));
    Ok(result)
}

fn is_end_of_track(kind: &EventKind) -> bool {
    matches!(kind, EventKind::Meta { type_id: 0x2F, .. })
}

fn is_tempo_event(kind: &EventKind) -> bool {
    matches!(kind, EventKind::Meta { type_id: 0x51, .. })
}

fn midly_kind_from_event<'a>(kind: &'a EventKind) -> Result<TrackEventKind<'a>, Error> {
    match kind {
        EventKind::Midi(bytes) => {
            let mut data = &bytes[..];
            parse_midi_event(&mut data)
        }
        EventKind::Meta { type_id, data } => {
            Ok(TrackEventKind::Meta(meta_from_payload(*type_id, data)?))
        }
        EventKind::Sysex(sysex) => match sysex.status {
            0xF0 => Ok(TrackEventKind::SysEx(&sysex.data)),
            0xF7 => Ok(TrackEventKind::Escape(&sysex.data)),
            _ => Err(Error::Invalid("invalid SysEx status")),
        },
        EventKind::Osc(_) => Err(Error::Unsupported(
            "OSC events cannot be represented in Standard MIDI Files",
        )),
        EventKind::Custom { .. } => Err(Error::Unsupported(
            "custom events cannot be represented in Standard MIDI Files",
        )),
    }
}

fn smpte_fps_from_midly(fps: Fps) -> SmpteFps {
    match fps {
        Fps::Fps24 => SmpteFps::Fps24,
        Fps::Fps25 => SmpteFps::Fps25,
        Fps::Fps29 => SmpteFps::Fps29Drop,
        Fps::Fps30 => SmpteFps::Fps30,
    }
}

fn smpte_fps_to_midly(fps: SmpteFps) -> Fps {
    match fps {
        SmpteFps::Fps24 => Fps::Fps24,
        SmpteFps::Fps25 => Fps::Fps25,
        SmpteFps::Fps29Drop => Fps::Fps29,
        SmpteFps::Fps30 => Fps::Fps30,
    }
}

fn smpte_ticks_to_absolute_delta(
    ticks: u64,
    timing: SmpteTiming,
    unit: AbsoluteUnit,
    remainder: &mut u128,
) -> Result<u64, Error> {
    let (fps_numerator, fps_denominator) = timing.fps.ratio();
    let units_per_second = match unit {
        AbsoluteUnit::Microseconds => 1_000_000u128,
        AbsoluteUnit::Nanoseconds => 1_000_000_000u128,
    };
    let numerator = u128::from(ticks)
        .checked_mul(units_per_second)
        .and_then(|value| value.checked_mul(u128::from(fps_denominator)))
        .and_then(|value| value.checked_add(*remainder))
        .ok_or(Error::DataOverflow("SMPTE conversion exceeds u128"))?;
    let denominator = u128::from(fps_numerator) * u128::from(timing.subframes);
    let delta = numerator / denominator;
    *remainder = numerator % denominator;
    u64::try_from(delta).map_err(|_| Error::DataOverflow("absolute delta exceeds u64"))
}

fn absolute_to_smpte_ticks(
    position: u64,
    timing: SmpteTiming,
    unit: AbsoluteUnit,
) -> Result<u64, Error> {
    let (fps_numerator, fps_denominator) = timing.fps.ratio();
    let units_per_second = match unit {
        AbsoluteUnit::Microseconds => 1_000_000u128,
        AbsoluteUnit::Nanoseconds => 1_000_000_000u128,
    };
    let numerator = u128::from(position)
        .checked_mul(u128::from(fps_numerator))
        .and_then(|value| value.checked_mul(u128::from(timing.subframes)))
        .ok_or(Error::DataOverflow("SMPTE conversion exceeds u128"))?;
    let denominator = units_per_second * u128::from(fps_denominator);
    let rounded = numerator
        .checked_add(denominator / 2)
        .ok_or(Error::DataOverflow("SMPTE rounding exceeds u128"))?
        / denominator;
    u64::try_from(rounded).map_err(|_| Error::DataOverflow("SMPTE position exceeds u64"))
}

#[cfg(test)]
const FLAG_SYSEX_STATUS_IN_PAYLOAD: u16 = 0x0001;

#[cfg(test)]
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
    let flags = u16::from_le_bytes([tsq_data[12], tsq_data[13]]);

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
            let events = parse_track(chunk_data, flags & FLAG_SYSEX_STATUS_IN_PAYLOAD != 0)?;
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

#[cfg(test)]
fn parse_track<'a>(
    mut data: &'a [u8],
    sysex_with_status: bool,
) -> Result<Vec<TrackEvent<'a>>, Error> {
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
            0x03 => parse_sysex_event(&mut data, sysex_with_status)?,
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
    let channel = u4::try_from(status & 0x0F).ok_or(Error::Invalid("invalid MIDI channel"))?;
    let high_nibble = status >> 4;
    let message = match high_nibble {
        0x8 => {
            let data1 = read_u8(data)?;
            let data2 = read_u8(data)?;
            MidiMessage::NoteOff {
                key: u7::try_from(data1).ok_or(Error::Invalid("note key out of range"))?,
                vel: u7::try_from(data2).ok_or(Error::Invalid("velocity out of range"))?,
            }
        }
        0x9 => {
            let data1 = read_u8(data)?;
            let data2 = read_u8(data)?;
            MidiMessage::NoteOn {
                key: u7::try_from(data1).ok_or(Error::Invalid("note key out of range"))?,
                vel: u7::try_from(data2).ok_or(Error::Invalid("velocity out of range"))?,
            }
        }
        0xA => {
            let data1 = read_u8(data)?;
            let data2 = read_u8(data)?;
            MidiMessage::Aftertouch {
                key: u7::try_from(data1).ok_or(Error::Invalid("note key out of range"))?,
                vel: u7::try_from(data2).ok_or(Error::Invalid("velocity out of range"))?,
            }
        }
        0xB => {
            let data1 = read_u8(data)?;
            let data2 = read_u8(data)?;
            MidiMessage::Controller {
                controller: u7::try_from(data1).ok_or(Error::Invalid("controller out of range"))?,
                value: u7::try_from(data2)
                    .ok_or(Error::Invalid("controller value out of range"))?,
            }
        }
        0xC => {
            let program = read_u8(data)?;
            MidiMessage::ProgramChange {
                program: u7::try_from(program).ok_or(Error::Invalid("program out of range"))?,
            }
        }
        0xD => {
            let vel = read_u8(data)?;
            MidiMessage::ChannelAftertouch {
                vel: u7::try_from(vel).ok_or(Error::Invalid("aftertouch velocity out of range"))?,
            }
        }
        0xE => {
            let data1 = read_u8(data)?;
            let data2 = read_u8(data)?;
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

#[cfg(test)]
fn parse_meta_event<'a>(data: &mut &'a [u8]) -> Result<TrackEventKind<'a>, Error> {
    let ty = read_u8(data)?;
    let len = read_vlq(data)?;
    let len_usize =
        usize::try_from(len).map_err(|_| Error::DataOverflow("meta payload too large"))?;
    let payload = take_slice(data, len_usize)?;
    let meta = meta_from_payload(ty, payload)?;
    Ok(TrackEventKind::Meta(meta))
}

#[cfg(test)]
fn parse_sysex_event<'a>(
    data: &mut &'a [u8],
    has_status: bool,
) -> Result<TrackEventKind<'a>, Error> {
    let len = read_vlq(data)?;
    let len_usize =
        usize::try_from(len).map_err(|_| Error::DataOverflow("sysex payload too large"))?;
    let payload = take_slice(data, len_usize)?;
    if has_status {
        let (status, body) = payload
            .split_first()
            .ok_or(Error::Invalid("sysex payload missing status byte"))?;
        match status {
            0xF0 => Ok(TrackEventKind::SysEx(body)),
            0xF7 => Ok(TrackEventKind::Escape(body)),
            _ => Err(Error::Invalid("invalid sysex status byte")),
        }
    } else {
        Ok(TrackEventKind::SysEx(payload))
    }
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
            if data.len() != 1 {
                return Err(Error::Invalid("MIDI channel meta must be 1 byte"));
            }
            let channel = data[0];
            let channel =
                u4::try_from(channel).ok_or(Error::Invalid("MIDI channel out of range"))?;
            MidiChannel(channel)
        }
        0x21 => {
            if data.len() != 1 {
                return Err(Error::Invalid("MIDI port meta must be 1 byte"));
            }
            let port = data[0];
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
            let is_minor = match data[1] {
                0 => false,
                1 => true,
                _ => return Err(Error::Invalid("key signature mode must be 0 or 1")),
            };
            KeySignature(sharps, is_minor)
        }
        0x7F => SequencerSpecific(data),
        _ => Unknown(ty, data),
    })
}

fn read_u8(data: &mut &[u8]) -> Result<u8, Error> {
    if data.is_empty() {
        return Err(Error::Invalid("unexpected end of track data"));
    }
    let byte = data[0];
    *data = &data[1..];
    Ok(byte)
}

#[cfg(test)]
fn take_slice<'a>(data: &mut &'a [u8], len: usize) -> Result<&'a [u8], Error> {
    if data.len() < len {
        return Err(Error::Invalid("payload exceeds remaining track data"));
    }
    let (prefix, rest) = data.split_at(len);
    *data = rest;
    Ok(prefix)
}

#[cfg(test)]
fn read_vlq(data: &mut &[u8]) -> Result<u64, Error> {
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

#[cfg(test)]
fn write_header(out: &mut Vec<u8>, ppq: u16, track_count: u16, flags: u16) {
    out.extend_from_slice(b"TSQ1");
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&ppq.to_le_bytes());
    out.push(0); // AbsUnit = microseconds
    out.push(0); // Reserved
    out.extend_from_slice(&track_count.to_le_bytes());
    out.extend_from_slice(&flags.to_le_bytes());
}

#[cfg(test)]
fn encode_event(
    delta: u64,
    kind: &TrackEventKind<'_>,
    out: &mut Vec<u8>,
    sysex_with_status: bool,
) -> Result<(), Error> {
    const DOMAIN_MUSICAL: u8 = 0;

    match kind {
        TrackEventKind::Midi { channel, message } => {
            out.push(DOMAIN_MUSICAL | 0x01);
            write_vlq(delta, out);
            let status = midi_status_byte(*channel, message);
            let (data1, data2) = midi_message_bytes(message);
            out.push(status);
            out.push(data1);
            if let Some(d2) = data2 {
                out.push(d2);
            }
        }
        TrackEventKind::SysEx(data) => {
            out.push(DOMAIN_MUSICAL | 0x03);
            write_vlq(delta, out);
            let len = data.len() + usize::from(sysex_with_status);
            write_vlq(len as u64, out);
            if sysex_with_status {
                out.push(0xF0);
            }
            out.extend_from_slice(data);
        }
        TrackEventKind::Escape(data) => {
            out.push(DOMAIN_MUSICAL | 0x03);
            write_vlq(delta, out);
            let len = data.len() + usize::from(sysex_with_status);
            write_vlq(len as u64, out);
            if sysex_with_status {
                out.push(0xF7);
            }
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

fn midi_message_bytes(message: &MidiMessage) -> (u8, Option<u8>) {
    match message {
        MidiMessage::NoteOff { key, vel }
        | MidiMessage::NoteOn { key, vel }
        | MidiMessage::Aftertouch { key, vel } => (key.as_int(), Some(vel.as_int())),
        MidiMessage::Controller { controller, value } => {
            (controller.as_int(), Some(value.as_int()))
        }
        MidiMessage::ProgramChange { program } => (program.as_int(), None),
        MidiMessage::ChannelAftertouch { vel } => (vel.as_int(), None),
        MidiMessage::PitchBend { bend } => {
            let raw = bend.0.as_int();
            ((raw & 0x7F) as u8, Some(((raw >> 7) & 0x7F) as u8))
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

#[cfg(test)]
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
#[cfg(feature = "std")]
pub mod ffi {
    use super::*;
    use alloc::vec::Vec;
    use core::{mem, ptr, slice};

    /// Buffer returned by the FFI conversion helpers.
    #[repr(C)]
    pub struct Tsq1Buffer {
        /// Pointer to the allocated bytes.
        pub ptr: *mut u8,
        /// Number of initialized bytes.
        pub len: usize,
        /// Capacity of the allocation.
        pub capacity: usize,
    }

    /// Status codes returned by the FFI API.
    #[repr(C)]
    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    pub enum Tsq1Status {
        /// Conversion completed successfully.
        Ok = 0,
        /// A required input or output pointer was null.
        NullPointer = 1,
        /// The input could not be converted.
        ConversionError = 2,
    }

    /// Convert SMF bytes into TSQ1 format, allocating a new buffer for the result.
    ///
    /// The caller is responsible for freeing the resulting buffer with [`tsq1_buffer_free`].
    ///
    /// # Safety
    ///
    /// `midi_ptr` must point to `midi_len` readable bytes, and `out` must point
    /// to writable storage for one [`Tsq1Buffer`]. The returned allocation must
    /// be released exactly once with [`tsq1_buffer_free`].
    pub unsafe extern "C" fn tsq1_mid_to_tsq(
        midi_ptr: *const u8,
        midi_len: usize,
        out: *mut Tsq1Buffer,
    ) -> Tsq1Status {
        if midi_ptr.is_null() || out.is_null() {
            return Tsq1Status::NullPointer;
        }
        unsafe {
            ptr::write(
                out,
                Tsq1Buffer {
                    ptr: ptr::null_mut(),
                    len: 0,
                    capacity: 0,
                },
            );
        }
        let midi = unsafe { slice::from_raw_parts(midi_ptr, midi_len) };
        match super::convert_midi_to_tsq_vec(midi) {
            Ok(mut data) => {
                let buffer = Tsq1Buffer {
                    ptr: data.as_mut_ptr(),
                    len: data.len(),
                    capacity: data.capacity(),
                };
                mem::forget(data);
                unsafe {
                    ptr::write(out, buffer);
                }
                Tsq1Status::Ok
            }
            Err(_) => Tsq1Status::ConversionError,
        }
    }

    /// Release a buffer produced by [`tsq1_mid_to_tsq`].
    ///
    /// # Safety
    ///
    /// `buf` must be either the untouched value returned by
    /// [`tsq1_mid_to_tsq`] or a buffer whose pointer is null. A non-null buffer
    /// must be passed exactly once.
    pub unsafe extern "C" fn tsq1_buffer_free(buf: Tsq1Buffer) {
        if buf.ptr.is_null() {
            return;
        }
        let _ = unsafe { Vec::from_raw_parts(buf.ptr, buf.len, buf.capacity) };
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

    fn build_tsq_track(
        events: &[TrackEventKind<'_>],
        deltas: &[u64],
        sysex_with_status: bool,
    ) -> Vec<u8> {
        assert_eq!(events.len(), deltas.len());
        let mut track = Vec::new();
        for (event, delta) in events.iter().zip(deltas.iter()) {
            encode_event(*delta, event, &mut track, sysex_with_status)
                .expect("encode_event should succeed");
        }
        track
    }

    fn tsq_with_single_track(track_data: &[u8], ppq: u16, flags: u16) -> Vec<u8> {
        let mut tsq = Vec::new();
        write_header(&mut tsq, ppq, 1, flags);
        tsq.extend_from_slice(b"TRK ");
        tsq.extend_from_slice(&(track_data.len() as u32).to_le_bytes());
        tsq.extend_from_slice(track_data);
        tsq
    }

    #[test]
    fn converts_simple_note_sequence_to_midi() {
        let channel = u4::try_from(0).unwrap();
        let key = u7::try_from(60).unwrap();
        let velocity = u7::try_from(100).unwrap();

        let events = [
            TrackEventKind::Midi {
                channel,
                message: MidiMessage::NoteOn { key, vel: velocity },
            },
            TrackEventKind::Midi {
                channel,
                message: MidiMessage::NoteOff { key, vel: velocity },
            },
            TrackEventKind::Meta(MetaMessage::EndOfTrack),
        ];
        let deltas = [0, 480, 0];
        let track = build_tsq_track(&events, &deltas, false);
        let tsq = tsq_with_single_track(&track, 480, 0);

        let midi = convert_tsq_to_midi_vec(&tsq).expect("conversion succeeds");
        let smf = Smf::parse(&midi).expect("generated MIDI parses");

        assert_eq!(smf.tracks.len(), 1);
        match smf.header.timing {
            Timing::Metrical(ppq) => assert_eq!(ppq.as_int(), 480),
            _ => panic!("expected metrical timing"),
        }
        assert_eq!(smf.header.format, Format::SingleTrack);

        let track = &smf.tracks[0];
        assert_eq!(track.len(), 3);
        assert_eq!(track[0].delta.as_int(), 0);
        assert_eq!(track[1].delta.as_int(), 480);
        assert_eq!(track[2].delta.as_int(), 0);

        match &track[0].kind {
            TrackEventKind::Midi {
                channel: ch,
                message,
            } => {
                assert_eq!(ch.as_int(), 0);
                match message {
                    MidiMessage::NoteOn { key: k, vel: v } => {
                        assert_eq!(k.as_int(), 60);
                        assert_eq!(v.as_int(), 100);
                    }
                    _ => panic!("expected note on"),
                }
            }
            _ => panic!("expected MIDI event"),
        }

        match &track[1].kind {
            TrackEventKind::Midi {
                channel: ch,
                message,
            } => {
                assert_eq!(ch.as_int(), 0);
                match message {
                    MidiMessage::NoteOff { key: k, vel: v } => {
                        assert_eq!(k.as_int(), 60);
                        assert_eq!(v.as_int(), 100);
                    }
                    _ => panic!("expected note off"),
                }
            }
            _ => panic!("expected MIDI event"),
        }

        assert!(matches!(
            &track[2].kind,
            TrackEventKind::Meta(MetaMessage::EndOfTrack)
        ));
    }

    #[test]
    fn parses_single_byte_midi_events() {
        let channel = u4::try_from(2).unwrap();
        let program = u7::try_from(12).unwrap();
        let aftertouch = u7::try_from(55).unwrap();

        let events = [
            TrackEventKind::Midi {
                channel,
                message: MidiMessage::ProgramChange { program },
            },
            TrackEventKind::Midi {
                channel,
                message: MidiMessage::ChannelAftertouch { vel: aftertouch },
            },
            TrackEventKind::Meta(MetaMessage::EndOfTrack),
        ];
        let deltas = [0, 12, 0];
        let track = build_tsq_track(&events, &deltas, false);
        let tsq = tsq_with_single_track(&track, 960, 0);

        let smf = super::convert_tsq_to_smf(&tsq).expect("conversion succeeds");
        assert_eq!(smf.tracks.len(), 1);
        let track = &smf.tracks[0];
        assert_eq!(track.len(), 3);

        match &track[0].kind {
            TrackEventKind::Midi {
                channel: ch,
                message,
            } => {
                assert_eq!(ch.as_int(), 2);
                match message {
                    MidiMessage::ProgramChange { program: p } => {
                        assert_eq!(p.as_int(), 12);
                    }
                    _ => panic!("expected program change"),
                }
            }
            _ => panic!("expected MIDI event"),
        }

        assert_eq!(track[1].delta.as_int(), 12);
        match &track[1].kind {
            TrackEventKind::Midi {
                channel: ch,
                message,
            } => {
                assert_eq!(ch.as_int(), 2);
                match message {
                    MidiMessage::ChannelAftertouch { vel } => {
                        assert_eq!(vel.as_int(), 55);
                    }
                    _ => panic!("expected channel aftertouch"),
                }
            }
            _ => panic!("expected MIDI event"),
        }

        match &track[2].kind {
            TrackEventKind::Meta(MetaMessage::EndOfTrack) => {}
            _ => panic!("expected end of track"),
        }
    }

    #[test]
    fn parses_sysex_and_escape_events() {
        let sysex_body: &[u8] = &[0x01, 0x02, 0x03];
        let escape_body: &[u8] = &[0x7D, 0x7E];

        let events = [
            TrackEventKind::SysEx(sysex_body),
            TrackEventKind::Escape(escape_body),
            TrackEventKind::Meta(MetaMessage::EndOfTrack),
        ];
        let deltas = [0, 90, 0];
        let track = build_tsq_track(&events, &deltas, true);
        let tsq = tsq_with_single_track(&track, 960, super::FLAG_SYSEX_STATUS_IN_PAYLOAD);

        let smf = super::convert_tsq_to_smf(&tsq).expect("conversion succeeds");
        assert_eq!(smf.tracks.len(), 1);
        let track = &smf.tracks[0];
        assert_eq!(track.len(), 3);

        match &track[0].kind {
            TrackEventKind::SysEx(data) => assert_eq!(*data, sysex_body),
            _ => panic!("expected sysex event"),
        }

        assert_eq!(track[1].delta.as_int(), 90);
        match &track[1].kind {
            TrackEventKind::Escape(data) => assert_eq!(*data, escape_body),
            _ => panic!("expected escape event"),
        }

        match &track[2].kind {
            TrackEventKind::Meta(MetaMessage::EndOfTrack) => {}
            _ => panic!("expected end of track"),
        }
    }

    #[test]
    fn midi_roundtrip_preserves_sysex_escape() {
        let track = vec![
            TrackEvent {
                delta: 0.into(),
                kind: TrackEventKind::SysEx(&[0x10, 0x20]),
            },
            TrackEvent {
                delta: u28::from(24u32),
                kind: TrackEventKind::Escape(&[0x30, 0x40, 0x41]),
            },
            TrackEvent {
                delta: 0.into(),
                kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
            },
        ];
        let smf = Smf {
            header: Header::new(Format::SingleTrack, Timing::Metrical(u15::from(480u16))),
            tracks: vec![track],
        };

        let mut midi_bytes = Vec::new();
        smf.write(&mut midi_bytes).expect("writing SMF succeeds");

        let tsq = super::convert_midi_to_tsq_vec(&midi_bytes).expect("TSQ conversion succeeds");
        let flags = u16::from_le_bytes([tsq[12], tsq[13]]);
        assert_eq!(
            flags & super::FLAG_SYSEX_STATUS_IN_PAYLOAD,
            super::FLAG_SYSEX_STATUS_IN_PAYLOAD
        );

        let midi_roundtrip = super::convert_tsq_to_midi_vec(&tsq).expect("roundtrip succeeds");
        let smf_roundtrip = Smf::parse(&midi_roundtrip).expect("parsed MIDI");
        assert_eq!(smf_roundtrip.tracks.len(), 1);
        let track = &smf_roundtrip.tracks[0];
        assert_eq!(track.len(), 3);

        match &track[0].kind {
            TrackEventKind::SysEx(data) => assert_eq!(*data, &[0x10, 0x20][..]),
            _ => panic!("expected sysex start"),
        }

        assert_eq!(track[1].delta.as_int(), 24);
        match &track[1].kind {
            TrackEventKind::Escape(data) => assert_eq!(*data, &[0x30, 0x40, 0x41][..]),
            _ => panic!("expected escape continuation"),
        }

        match &track[2].kind {
            TrackEventKind::Meta(MetaMessage::EndOfTrack) => {}
            _ => panic!("expected end of track"),
        }
    }

    #[test]
    fn midi_without_sysex_does_not_set_status_flag() {
        let channel = u4::try_from(0).unwrap();
        let key = u7::try_from(64).unwrap();
        let velocity = u7::try_from(90).unwrap();
        let duration = u28::from(120u32);

        let track = vec![
            TrackEvent {
                delta: 0.into(),
                kind: TrackEventKind::Midi {
                    channel,
                    message: MidiMessage::NoteOn { key, vel: velocity },
                },
            },
            TrackEvent {
                delta: duration,
                kind: TrackEventKind::Midi {
                    channel,
                    message: MidiMessage::NoteOff { key, vel: velocity },
                },
            },
            TrackEvent {
                delta: 0.into(),
                kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
            },
        ];

        let smf = Smf {
            header: Header::new(Format::SingleTrack, Timing::Metrical(u15::from(960u16))),
            tracks: vec![track],
        };

        let mut midi_bytes = Vec::new();
        smf.write(&mut midi_bytes).expect("writing SMF succeeds");

        let tsq = super::convert_midi_to_tsq_vec(&midi_bytes).expect("conversion succeeds");
        let flags = u16::from_le_bytes([tsq[12], tsq[13]]);
        assert_eq!(flags & super::FLAG_SYSEX_STATUS_IN_PAYLOAD, 0);
    }

    #[test]
    fn same_tick_tempo_map_keeps_the_later_event() {
        let track = vec![
            TrackEvent {
                delta: 0.into(),
                kind: TrackEventKind::Meta(MetaMessage::Tempo(u24::from(600_000))),
            },
            TrackEvent {
                delta: 0.into(),
                kind: TrackEventKind::Meta(MetaMessage::Tempo(u24::from(400_000))),
            },
            TrackEvent {
                delta: 0.into(),
                kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
            },
        ];
        let smf = Smf {
            header: Header::new(Format::SingleTrack, Timing::Metrical(u15::from(480))),
            tracks: vec![track],
        };

        let sequence = super::sequence_from_smf(&smf).expect("conversion succeeds");
        assert_eq!(
            sequence.tempo_map,
            vec![TempoEntry {
                tick: 0,
                microseconds_per_quarter: 400_000,
            }]
        );
    }

    #[test]
    fn metrical_export_synthesizes_the_canonical_tempo_map() {
        let mut sequence = Sequence::new(480);
        sequence.tempo_map = vec![
            TempoEntry {
                tick: 0,
                microseconds_per_quarter: 500_000,
            },
            TempoEntry {
                tick: 480,
                microseconds_per_quarter: 400_000,
            },
        ];
        sequence.tracks.push(Track {
            events: vec![
                Event {
                    delta: 240,
                    domain: TimeDomain::Musical,
                    kind: EventKind::Midi([0x90, 60, 100]),
                },
                Event {
                    delta: 0,
                    domain: TimeDomain::Musical,
                    kind: EventKind::Meta {
                        type_id: 0x2F,
                        data: Vec::new(),
                    },
                },
            ],
        });

        let midi = convert_tsq_to_midi_vec(&sequence.encode().unwrap()).expect("convert");
        let parsed = Smf::parse(&midi).expect("parse");
        let track = &parsed.tracks[0];
        assert_eq!(track.len(), 4);
        assert_eq!(track[0].delta.as_int(), 0);
        assert!(matches!(
            track[0].kind,
            TrackEventKind::Meta(MetaMessage::Tempo(value)) if value.as_int() == 500_000
        ));
        assert_eq!(track[1].delta.as_int(), 240);
        assert!(matches!(track[1].kind, TrackEventKind::Midi { .. }));
        assert_eq!(track[2].delta.as_int(), 240);
        assert!(matches!(
            track[2].kind,
            TrackEventKind::Meta(MetaMessage::Tempo(value)) if value.as_int() == 400_000
        ));
        assert_eq!(track[3].delta.as_int(), 0);
        assert!(matches!(
            track[3].kind,
            TrackEventKind::Meta(MetaMessage::EndOfTrack)
        ));
    }

    #[test]
    fn canonical_tempo_map_replaces_redundant_track_events() {
        let mut sequence = Sequence::new(480);
        sequence.tempo_map.push(TempoEntry {
            tick: 0,
            microseconds_per_quarter: 500_000,
        });
        sequence.tracks.push(Track {
            events: vec![
                Event {
                    delta: 0,
                    domain: TimeDomain::Musical,
                    kind: EventKind::Meta {
                        type_id: 0x51,
                        data: vec![0x09, 0x27, 0xC0],
                    },
                },
                Event {
                    delta: 0,
                    domain: TimeDomain::Musical,
                    kind: EventKind::Meta {
                        type_id: 0x2F,
                        data: Vec::new(),
                    },
                },
            ],
        });

        let midi = convert_tsq_to_midi_vec(&sequence.encode().unwrap()).expect("convert");
        let parsed = Smf::parse(&midi).expect("parse");
        assert_eq!(parsed.tracks[0].len(), 2);
        assert!(matches!(
            parsed.tracks[0][0].kind,
            TrackEventKind::Meta(MetaMessage::Tempo(value)) if value.as_int() == 500_000
        ));
        assert!(matches!(
            parsed.tracks[0][1].kind,
            TrackEventKind::Meta(MetaMessage::EndOfTrack)
        ));
    }

    #[test]
    fn tempo_only_sequence_exports_a_conductor_track() {
        let mut sequence = Sequence::new(480);
        sequence.tempo_map.push(TempoEntry {
            tick: 0,
            microseconds_per_quarter: 500_000,
        });

        let midi = convert_tsq_to_midi_vec(&sequence.encode().unwrap()).expect("convert");
        let parsed = Smf::parse(&midi).expect("parse");
        assert_eq!(parsed.tracks.len(), 1);
        assert_eq!(parsed.tracks[0].len(), 2);
        assert!(matches!(
            parsed.tracks[0][0].kind,
            TrackEventKind::Meta(MetaMessage::Tempo(value)) if value.as_int() == 500_000
        ));
        assert!(matches!(
            parsed.tracks[0][1].kind,
            TrackEventKind::Meta(MetaMessage::EndOfTrack)
        ));
    }

    #[test]
    fn mixed_domain_export_moves_end_of_track_after_the_timeline() {
        let mut sequence = Sequence::new(480);
        sequence.sync_anchors = vec![
            SyncAnchor { tick: 0, time: 0 },
            SyncAnchor {
                tick: 960,
                time: 1_000_000,
            },
        ];
        sequence.tracks.push(Track {
            events: vec![
                Event {
                    delta: 500_000,
                    domain: TimeDomain::Absolute,
                    kind: EventKind::Midi([0x90, 60, 100]),
                },
                Event {
                    delta: 240,
                    domain: TimeDomain::Musical,
                    kind: EventKind::Meta {
                        type_id: 0x2F,
                        data: Vec::new(),
                    },
                },
            ],
        });

        let midi = convert_tsq_to_midi_vec(&sequence.encode().unwrap()).expect("convert");
        let parsed = Smf::parse(&midi).expect("parse");
        let track = &parsed.tracks[0];
        assert_eq!(track[0].delta.as_int(), 480);
        assert!(matches!(track[0].kind, TrackEventKind::Midi { .. }));
        assert_eq!(track[1].delta.as_int(), 0);
        assert!(matches!(
            track[1].kind,
            TrackEventKind::Meta(MetaMessage::EndOfTrack)
        ));
    }

    #[test]
    fn fixed_width_channel_and_port_meta_reject_extra_bytes() {
        for (type_id, expected) in [
            (0x20, "MIDI channel meta must be 1 byte"),
            (0x21, "MIDI port meta must be 1 byte"),
        ] {
            let mut sequence = Sequence::new(480);
            sequence.tracks.push(Track {
                events: vec![Event {
                    delta: 0,
                    domain: TimeDomain::Musical,
                    kind: EventKind::Meta {
                        type_id,
                        data: vec![1, 2],
                    },
                }],
            });

            let error = convert_tsq_to_midi_vec(&sequence.encode().unwrap()).unwrap_err();
            assert!(matches!(error, Error::Invalid(message) if message == expected));
        }
    }

    #[test]
    fn key_signature_rejects_invalid_mode_byte() {
        let mut sequence = Sequence::new(480);
        sequence.tracks.push(Track {
            events: vec![Event {
                delta: 0,
                domain: TimeDomain::Musical,
                kind: EventKind::Meta {
                    type_id: 0x59,
                    data: vec![0, 2],
                },
            }],
        });

        let error = convert_tsq_to_midi_vec(&sequence.encode().unwrap()).unwrap_err();
        assert!(matches!(
            error,
            Error::Invalid("key signature mode must be 0 or 1")
        ));
    }

    #[test]
    fn midi_roundtrip_matches_original_content() {
        let conductor_name: &[u8] = b"Conductor";
        let lead_name: &[u8] = b"Lead";
        let tempo = u24::from(500_000u32);

        let track0 = vec![
            TrackEvent {
                delta: 0.into(),
                kind: TrackEventKind::Meta(MetaMessage::TrackName(conductor_name)),
            },
            TrackEvent {
                delta: 0.into(),
                kind: TrackEventKind::Meta(MetaMessage::TimeSignature(4, 2, 24, 8)),
            },
            TrackEvent {
                delta: 0.into(),
                kind: TrackEventKind::Meta(MetaMessage::Tempo(tempo)),
            },
            TrackEvent {
                delta: 0.into(),
                kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
            },
        ];

        let channel = u4::try_from(1).unwrap();
        let key = u7::try_from(67).unwrap();
        let velocity = u7::try_from(110).unwrap();

        let track1 = vec![
            TrackEvent {
                delta: 0.into(),
                kind: TrackEventKind::Meta(MetaMessage::TrackName(lead_name)),
            },
            TrackEvent {
                delta: u28::from(120u32),
                kind: TrackEventKind::Midi {
                    channel,
                    message: MidiMessage::NoteOn { key, vel: velocity },
                },
            },
            TrackEvent {
                delta: u28::from(480u32),
                kind: TrackEventKind::Midi {
                    channel,
                    message: MidiMessage::NoteOff { key, vel: velocity },
                },
            },
            TrackEvent {
                delta: 0.into(),
                kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
            },
        ];

        let smf = Smf {
            header: Header::new(Format::Parallel, Timing::Metrical(u15::from(960u16))),
            tracks: vec![track0, track1],
        };

        let mut midi_bytes = Vec::new();
        smf.write(&mut midi_bytes).expect("writing SMF succeeds");

        let tsq = super::convert_midi_to_tsq_vec(&midi_bytes).expect("TSQ conversion succeeds");
        let midi_roundtrip =
            super::convert_tsq_to_midi_vec(&tsq).expect("roundtrip conversion succeeds");

        let smf_roundtrip = Smf::parse(&midi_roundtrip).expect("roundtrip MIDI parses");
        assert_eq!(smf_roundtrip.header.format, Format::Parallel);
        match smf_roundtrip.header.timing {
            Timing::Metrical(ppq) => assert_eq!(ppq.as_int(), 960),
            _ => panic!("expected metrical timing"),
        }

        assert_eq!(smf_roundtrip.tracks.len(), 2);

        let roundtrip_track0 = &smf_roundtrip.tracks[0];
        assert_eq!(roundtrip_track0.len(), 4);
        match &roundtrip_track0[0].kind {
            TrackEventKind::Meta(MetaMessage::TrackName(name)) => {
                assert_eq!(*name, b"Conductor")
            }
            other => panic!("unexpected first meta event: {other:?}"),
        }
        match &roundtrip_track0[1].kind {
            TrackEventKind::Meta(MetaMessage::TimeSignature(num, denom, clocks, notes)) => {
                assert_eq!((*num, *denom, *clocks, *notes), (4, 2, 24, 8));
            }
            other => panic!("unexpected time signature event: {other:?}"),
        }
        match &roundtrip_track0[2].kind {
            TrackEventKind::Meta(MetaMessage::Tempo(value)) => {
                assert_eq!(value.as_int(), 500_000);
            }
            other => panic!("unexpected tempo event: {other:?}"),
        }
        assert!(matches!(
            &roundtrip_track0[3].kind,
            TrackEventKind::Meta(MetaMessage::EndOfTrack)
        ));

        let roundtrip_track1 = &smf_roundtrip.tracks[1];
        assert_eq!(roundtrip_track1.len(), 4);
        match &roundtrip_track1[0].kind {
            TrackEventKind::Meta(MetaMessage::TrackName(name)) => {
                assert_eq!(*name, b"Lead")
            }
            other => panic!("unexpected track name event: {other:?}"),
        }
        assert_eq!(roundtrip_track1[1].delta.as_int(), 120);
        match &roundtrip_track1[1].kind {
            TrackEventKind::Midi {
                channel: ch,
                message,
            } => {
                assert_eq!(ch.as_int(), 1);
                match message {
                    MidiMessage::NoteOn { key: note, vel } => {
                        assert_eq!(note.as_int(), 67);
                        assert_eq!(vel.as_int(), 110);
                    }
                    other => panic!("unexpected message for note on: {other:?}"),
                }
            }
            other => panic!("unexpected second track event: {other:?}"),
        }
        assert_eq!(roundtrip_track1[2].delta.as_int(), 480);
        match &roundtrip_track1[2].kind {
            TrackEventKind::Midi {
                channel: ch,
                message,
            } => {
                assert_eq!(ch.as_int(), 1);
                match message {
                    MidiMessage::NoteOff { key: note, vel } => {
                        assert_eq!(note.as_int(), 67);
                        assert_eq!(vel.as_int(), 110);
                    }
                    other => panic!("unexpected message for note off: {other:?}"),
                }
            }
            other => panic!("unexpected third track event: {other:?}"),
        }
        assert!(matches!(
            &roundtrip_track1[3].kind,
            TrackEventKind::Meta(MetaMessage::EndOfTrack)
        ));
    }

    #[test]
    fn smpte_divisions_roundtrip_through_absolute_events() {
        for fps in [Fps::Fps24, Fps::Fps25, Fps::Fps29, Fps::Fps30] {
            let track = vec![
                TrackEvent {
                    delta: 0.into(),
                    kind: TrackEventKind::Midi {
                        channel: u4::from(0),
                        message: MidiMessage::NoteOn {
                            key: u7::from(60),
                            vel: u7::from(100),
                        },
                    },
                },
                TrackEvent {
                    delta: u28::from(240),
                    kind: TrackEventKind::Midi {
                        channel: u4::from(0),
                        message: MidiMessage::NoteOff {
                            key: u7::from(60),
                            vel: u7::from(0),
                        },
                    },
                },
                TrackEvent {
                    delta: 0.into(),
                    kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
                },
            ];
            let smf = Smf {
                header: Header::new(Format::SingleTrack, Timing::Timecode(fps, 80)),
                tracks: vec![track],
            };
            let mut midi = Vec::new();
            smf.write(&mut midi).expect("write SMPTE SMF");

            let tsq = convert_midi_to_tsq_vec(&midi).expect("convert SMPTE SMF");
            let sequence = Sequence::decode(&tsq).expect("decode sequence");
            assert_eq!(
                sequence.smpte_timing,
                Some(SmpteTiming {
                    fps: smpte_fps_from_midly(fps),
                    subframes: 80,
                })
            );
            assert!(sequence.tracks[0]
                .events
                .iter()
                .all(|event| event.domain == TimeDomain::Absolute));

            let roundtrip = convert_tsq_to_midi_vec(&tsq).expect("convert back to SMPTE SMF");
            let parsed = Smf::parse(&roundtrip).expect("parse roundtrip");
            assert_eq!(parsed.header.timing, Timing::Timecode(fps, 80));
            assert_eq!(parsed.tracks[0][1].delta.as_int().abs_diff(240), 0);
        }
    }

    #[test]
    fn absolute_events_map_to_metrical_midi_with_sync_anchors() {
        let mut sequence = Sequence::new(480);
        sequence.sync_anchors = vec![
            SyncAnchor { tick: 0, time: 0 },
            SyncAnchor {
                tick: 960,
                time: 1_000_000,
            },
        ];
        sequence.tracks.push(Track {
            events: vec![
                Event {
                    delta: 500_000,
                    domain: TimeDomain::Absolute,
                    kind: EventKind::Midi([0x90, 60, 100]),
                },
                Event {
                    delta: 0,
                    domain: TimeDomain::Absolute,
                    kind: EventKind::Meta {
                        type_id: 0x2F,
                        data: Vec::new(),
                    },
                },
            ],
        });

        let midi = convert_tsq_to_midi_vec(&sequence.encode().unwrap()).expect("convert");
        let parsed = Smf::parse(&midi).expect("parse");
        assert_eq!(parsed.header.timing, Timing::Metrical(u15::from(480)));
        assert_eq!(parsed.tracks[0][0].delta.as_int(), 480);
    }

    #[test]
    fn non_midi_events_are_not_silently_dropped() {
        let mut sequence = Sequence::new(480);
        sequence.tracks.push(Track {
            events: vec![Event {
                delta: 0,
                domain: TimeDomain::Musical,
                kind: EventKind::Custom {
                    type_id: 1,
                    data: vec![1],
                },
            }],
        });
        let error = convert_tsq_to_midi_vec(&sequence.encode().unwrap()).unwrap_err();
        assert!(matches!(error, Error::Unsupported(_)));
    }
}
