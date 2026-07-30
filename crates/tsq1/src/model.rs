use alloc::string::String;
use alloc::vec::Vec;
use core::convert::TryFrom;

use crate::Error;

pub(crate) const FLAG_SYSEX_STATUS_IN_PAYLOAD: u16 = 0x0001;
pub(crate) const FLAG_FIXED_MIDI_WIDTH: u16 = 0x0002;

const HEADER_SIZE: usize = 14;
const VERSION: u16 = 1;
const CHUNK_TRACK: [u8; 4] = *b"TRK ";
const CHUNK_TEMPO: [u8; 4] = *b"TMAP";
const CHUNK_SYNC: [u8; 4] = *b"SYNC";
const CHUNK_MARKER: [u8; 4] = *b"MARK";
const CHUNK_SMPTE: [u8; 4] = *b"SMPF";

/// Unit used by absolute-domain deltas and positions.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum AbsoluteUnit {
    /// One unit is one microsecond.
    #[default]
    Microseconds = 0,
    /// One unit is one nanosecond.
    Nanoseconds = 1,
}

impl TryFrom<u8> for AbsoluteUnit {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Microseconds),
            1 => Ok(Self::Nanoseconds),
            _ => Err(Error::Invalid("unsupported absolute time unit")),
        }
    }
}

/// Time domain used by an event delta or marker position.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeDomain {
    /// PPQ-based musical ticks.
    Musical,
    /// Elapsed time in [`Sequence::absolute_unit`].
    Absolute,
}

/// OSC payload representation.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum OscFormat {
    /// Byte-accurate OSC 1.0/1.1 datagram.
    Raw = 0,
    /// MessagePack-encoded OSC intermediate representation.
    MessagePack = 1,
    /// CBOR-encoded OSC intermediate representation.
    Cbor = 2,
}

impl TryFrom<u8> for OscFormat {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Raw),
            1 => Ok(Self::MessagePack),
            2 => Ok(Self::Cbor),
            _ => Err(Error::Invalid("unsupported OSC payload format")),
        }
    }
}

/// OSC event payload.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OscEvent {
    /// Payload representation.
    pub format: OscFormat,
    /// Encoded OSC packet or intermediate representation.
    pub data: Vec<u8>,
}

/// System-exclusive event payload.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SysexEvent {
    /// MIDI status byte (`0xF0` for SysEx or `0xF7` for escape/continuation).
    pub status: u8,
    /// Payload excluding the status byte.
    pub data: Vec<u8>,
}

/// Event payload stored in a track.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(tag = "kind", content = "value", rename_all = "camelCase")
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventKind {
    /// OSC packet or intermediate representation.
    Osc(OscEvent),
    /// Full MIDI channel message: status, data byte 1, data byte 2.
    Midi([u8; 3]),
    /// SMF-compatible meta event.
    Meta {
        /// Meta type byte.
        type_id: u8,
        /// Meta payload.
        data: Vec<u8>,
    },
    /// System-exclusive or escape payload.
    Sysex(SysexEvent),
    /// Vendor-specific event.
    Custom {
        /// Vendor-defined type identifier.
        type_id: u8,
        /// Uninterpreted vendor payload.
        data: Vec<u8>,
    },
}

/// One event with a domain-specific delta.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event {
    /// Delta from the previous event in the same time domain.
    pub delta: u64,
    /// Domain in which `delta` is expressed.
    pub domain: TimeDomain,
    /// Event payload.
    pub kind: EventKind,
}

/// Ordered event stream.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Track {
    /// Events in storage order.
    pub events: Vec<Event>,
}

/// Tempo-map entry.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TempoEntry {
    /// Absolute musical tick.
    pub tick: u64,
    /// Microseconds per quarter note.
    pub microseconds_per_quarter: u32,
}

/// Anchor relating a musical tick to absolute elapsed time.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SyncAnchor {
    /// Absolute musical tick.
    pub tick: u64,
    /// Absolute elapsed time in the sequence's absolute unit.
    pub time: u64,
}

/// Human-readable timeline marker.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Marker {
    /// Position domain.
    pub domain: TimeDomain,
    /// Absolute position in the selected domain.
    pub position: u64,
    /// UTF-8 marker label.
    pub name: String,
    /// Marker class (`0x00` generic, `0x20` cue, `0x7F` custom).
    pub class: u8,
    /// Optional `0xAARRGGBB` color.
    pub color_rgba: Option<u32>,
}

/// SMPTE frame-rate division.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SmpteFps {
    /// 24 frames per second.
    Fps24 = 24,
    /// 25 frames per second.
    Fps25 = 25,
    /// 29.97 drop-frame (`30_000 / 1_001`) frames per second.
    Fps29Drop = 29,
    /// 30 frames per second.
    Fps30 = 30,
}

impl TryFrom<u8> for SmpteFps {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            24 => Ok(Self::Fps24),
            25 => Ok(Self::Fps25),
            29 => Ok(Self::Fps29Drop),
            30 => Ok(Self::Fps30),
            _ => Err(Error::Invalid("unsupported SMPTE frame rate")),
        }
    }
}

impl SmpteFps {
    /// Exact frame-rate numerator and denominator.
    pub const fn ratio(self) -> (u32, u32) {
        match self {
            Self::Fps24 => (24, 1),
            Self::Fps25 => (25, 1),
            Self::Fps29Drop => (30_000, 1_001),
            Self::Fps30 => (30, 1),
        }
    }
}

/// Original SMPTE timing division retained for MIDI round-trips.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SmpteTiming {
    /// Frames per second.
    pub fps: SmpteFps,
    /// MIDI subframes per frame.
    pub subframes: u8,
}

/// Forward-compatible chunk retained without interpretation.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnknownChunk {
    /// Four-byte chunk identifier.
    pub id: [u8; 4],
    /// Uninterpreted chunk payload.
    pub data: Vec<u8>,
}

/// Complete owned TSQ1 document.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sequence {
    /// Ticks per quarter note.
    pub ppq: u16,
    /// Absolute-time unit.
    pub absolute_unit: AbsoluteUnit,
    /// Header flags, including unknown forward-compatible bits.
    pub flags: u16,
    /// Event tracks.
    pub tracks: Vec<Track>,
    /// Tempo-map entries.
    pub tempo_map: Vec<TempoEntry>,
    /// Musical/absolute synchronization anchors.
    pub sync_anchors: Vec<SyncAnchor>,
    /// Navigation markers.
    pub markers: Vec<Marker>,
    /// Optional original SMPTE division.
    pub smpte_timing: Option<SmpteTiming>,
    /// Unknown chunks retained byte-for-byte.
    pub unknown_chunks: Vec<UnknownChunk>,
}

impl Default for Sequence {
    fn default() -> Self {
        Self::new(480)
    }
}

impl Sequence {
    /// Create an empty sequence using microsecond absolute timing.
    pub fn new(ppq: u16) -> Self {
        Self {
            ppq,
            absolute_unit: AbsoluteUnit::Microseconds,
            flags: FLAG_FIXED_MIDI_WIDTH,
            tracks: Vec::new(),
            tempo_map: Vec::new(),
            sync_anchors: Vec::new(),
            markers: Vec::new(),
            smpte_timing: None,
            unknown_chunks: Vec::new(),
        }
    }

    /// Decode and validate a TSQ1 document.
    pub fn decode(data: &[u8]) -> Result<Self, Error> {
        let mut cursor = Cursor::new(data, 0);
        if data.len() < HEADER_SIZE {
            return Err(cursor.error("TSQ1 header is truncated"));
        }
        if cursor.take(4)? != b"TSQ1" {
            return Err(cursor.error_at(0, "TSQ1 magic is missing"));
        }
        if cursor.read_u16()? != VERSION {
            return Err(cursor.error_at(4, "unsupported TSQ1 version"));
        }
        let ppq = cursor.read_u16()?;
        let unit_offset = cursor.position();
        let absolute_unit = AbsoluteUnit::try_from(cursor.read_u8()?)
            .map_err(|error| error_at(error, unit_offset))?;
        if cursor.read_u8()? != 0 {
            return Err(cursor.error_at(9, "header reserved byte must be zero"));
        }
        let _advisory_track_count = cursor.read_u16()?;
        let flags = cursor.read_u16()?;

        let mut sequence = Self {
            ppq,
            absolute_unit,
            flags,
            tracks: Vec::new(),
            tempo_map: Vec::new(),
            sync_anchors: Vec::new(),
            markers: Vec::new(),
            smpte_timing: None,
            unknown_chunks: Vec::new(),
        };

        while !cursor.is_empty() {
            let chunk_offset = cursor.position();
            let id_slice = cursor.take(4)?;
            let id = [id_slice[0], id_slice[1], id_slice[2], id_slice[3]];
            let length = usize::try_from(cursor.read_u32()?)
                .map_err(|_| cursor.error("chunk length exceeds platform limits"))?;
            let payload_offset = cursor.position();
            let payload = cursor.take(length)?;
            let mut chunk = Cursor::new(payload, payload_offset);
            match id {
                CHUNK_TRACK => {
                    sequence
                        .tracks
                        .push(decode_track(payload, payload_offset, flags)?);
                }
                CHUNK_TEMPO => {
                    if length % 12 != 0 {
                        return Err(
                            cursor.error_at(chunk_offset, "TMAP length is not a multiple of 12")
                        );
                    }
                    while !chunk.is_empty() {
                        sequence.tempo_map.push(TempoEntry {
                            tick: chunk.read_u64()?,
                            microseconds_per_quarter: chunk.read_u32()?,
                        });
                    }
                }
                CHUNK_SYNC => {
                    if length % 16 != 0 {
                        return Err(
                            cursor.error_at(chunk_offset, "SYNC length is not a multiple of 16")
                        );
                    }
                    while !chunk.is_empty() {
                        sequence.sync_anchors.push(SyncAnchor {
                            tick: chunk.read_u64()?,
                            time: chunk.read_u64()?,
                        });
                    }
                }
                CHUNK_MARKER => {
                    while !chunk.is_empty() {
                        let domain = decode_domain(chunk.read_u8()?, &chunk)?;
                        let position = chunk.read_u64()?;
                        let name_length = chunk.read_vlq_usize()?;
                        let name_offset = chunk.position();
                        let name_bytes = chunk.take(name_length)?;
                        let name = core::str::from_utf8(name_bytes)
                            .map_err(|_| chunk.error_at(name_offset, "marker name is not UTF-8"))?
                            .into();
                        let class = chunk.read_u8()?;
                        let marker_flags = chunk.read_u8()?;
                        if marker_flags & !0x01 != 0 {
                            return Err(chunk.error("unsupported MARK flags"));
                        }
                        let color_rgba = if marker_flags & 0x01 != 0 {
                            Some(chunk.read_u32()?)
                        } else {
                            None
                        };
                        sequence.markers.push(Marker {
                            domain,
                            position,
                            name,
                            class,
                            color_rgba,
                        });
                    }
                }
                CHUNK_SMPTE => {
                    if length != 2 || sequence.smpte_timing.is_some() {
                        return Err(cursor.error_at(
                            chunk_offset,
                            "SMPF must occur once with a two-byte payload",
                        ));
                    }
                    let fps_offset = chunk.position();
                    let fps = SmpteFps::try_from(chunk.read_u8()?)
                        .map_err(|error| error_at(error, fps_offset))?;
                    let subframes = chunk.read_u8()?;
                    sequence.smpte_timing = Some(SmpteTiming { fps, subframes });
                }
                _ => sequence.unknown_chunks.push(UnknownChunk {
                    id,
                    data: payload.to_vec(),
                }),
            }
        }

        sequence.validate().map_err(|error| error_at(error, 0))?;
        Ok(sequence)
    }

    /// Validate model invariants without encoding.
    pub fn validate(&self) -> Result<(), Error> {
        if self.ppq == 0 {
            return Err(Error::Invalid("PPQ must be greater than zero"));
        }
        if self.tracks.len() > u16::MAX as usize {
            return Err(Error::DataOverflow("too many tracks"));
        }
        if let Some(timing) = self.smpte_timing {
            if timing.subframes == 0 {
                return Err(Error::Invalid("SMPTE subframes must be greater than zero"));
            }
        }
        ensure_strictly_increasing(
            self.tempo_map.iter().map(|entry| entry.tick),
            "tempo map ticks must be strictly increasing",
        )?;
        ensure_strictly_increasing(
            self.sync_anchors.iter().map(|entry| entry.tick),
            "sync ticks must be strictly increasing",
        )?;
        ensure_strictly_increasing(
            self.sync_anchors.iter().map(|entry| entry.time),
            "sync absolute times must be strictly increasing",
        )?;

        let mut last_musical = None;
        let mut last_absolute = None;
        for marker in &self.markers {
            let previous = match marker.domain {
                TimeDomain::Musical => &mut last_musical,
                TimeDomain::Absolute => &mut last_absolute,
            };
            if previous.is_some_and(|value| marker.position < value) {
                return Err(Error::Invalid("markers must be sorted within each domain"));
            }
            *previous = Some(marker.position);
        }

        for track in &self.tracks {
            for event in &track.events {
                validate_event(event)?;
            }
        }
        for chunk in &self.unknown_chunks {
            if is_known_chunk(chunk.id) {
                return Err(Error::Invalid("known chunk cannot be stored as unknown"));
            }
            checked_u32_len(chunk.data.len(), "unknown chunk is too large")?;
        }
        Ok(())
    }

    /// Encode this sequence in canonical chunk order.
    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        self.validate()?;
        let has_sysex = self
            .tracks
            .iter()
            .flat_map(|track| &track.events)
            .any(|event| matches!(event.kind, EventKind::Sysex(_)));
        let mut flags = self.flags | FLAG_FIXED_MIDI_WIDTH;
        if has_sysex {
            flags |= FLAG_SYSEX_STATUS_IN_PAYLOAD;
        } else {
            flags &= !FLAG_SYSEX_STATUS_IN_PAYLOAD;
        }

        let mut out = Vec::new();
        out.extend_from_slice(b"TSQ1");
        out.extend_from_slice(&VERSION.to_le_bytes());
        out.extend_from_slice(&self.ppq.to_le_bytes());
        out.push(self.absolute_unit as u8);
        out.push(0);
        out.extend_from_slice(&(self.tracks.len() as u16).to_le_bytes());
        out.extend_from_slice(&flags.to_le_bytes());

        for track in &self.tracks {
            let mut payload = Vec::new();
            for event in &track.events {
                encode_event(event, &mut payload, has_sysex)?;
            }
            push_chunk(&mut out, CHUNK_TRACK, &payload)?;
        }
        if !self.tempo_map.is_empty() {
            let mut payload = Vec::new();
            for entry in &self.tempo_map {
                payload.extend_from_slice(&entry.tick.to_le_bytes());
                payload.extend_from_slice(&entry.microseconds_per_quarter.to_le_bytes());
            }
            push_chunk(&mut out, CHUNK_TEMPO, &payload)?;
        }
        if !self.sync_anchors.is_empty() {
            let mut payload = Vec::new();
            for anchor in &self.sync_anchors {
                payload.extend_from_slice(&anchor.tick.to_le_bytes());
                payload.extend_from_slice(&anchor.time.to_le_bytes());
            }
            push_chunk(&mut out, CHUNK_SYNC, &payload)?;
        }
        if !self.markers.is_empty() {
            let mut payload = Vec::new();
            for marker in &self.markers {
                payload.push(encode_domain(marker.domain));
                payload.extend_from_slice(&marker.position.to_le_bytes());
                write_vlq(marker.name.len() as u64, &mut payload);
                payload.extend_from_slice(marker.name.as_bytes());
                payload.push(marker.class);
                match marker.color_rgba {
                    Some(color) => {
                        payload.push(0x01);
                        payload.extend_from_slice(&color.to_le_bytes());
                    }
                    None => payload.push(0),
                }
            }
            push_chunk(&mut out, CHUNK_MARKER, &payload)?;
        }
        if let Some(timing) = self.smpte_timing {
            push_chunk(&mut out, CHUNK_SMPTE, &[timing.fps as u8, timing.subframes])?;
        }
        for chunk in &self.unknown_chunks {
            push_chunk(&mut out, chunk.id, &chunk.data)?;
        }
        Ok(out)
    }

    /// Convert a musical tick to absolute elapsed time using linear interpolation.
    pub fn tick_to_absolute(&self, tick: u64) -> Result<u64, Error> {
        interpolate_anchors(&self.sync_anchors, tick, true)
    }

    /// Convert absolute elapsed time to a musical tick using linear interpolation.
    pub fn absolute_to_tick(&self, time: u64) -> Result<u64, Error> {
        interpolate_anchors(&self.sync_anchors, time, false)
    }
}

fn decode_track(data: &[u8], base_offset: usize, flags: u16) -> Result<Track, Error> {
    let mut cursor = Cursor::new(data, base_offset);
    let mut events = Vec::new();
    while !cursor.is_empty() {
        let header = cursor.read_u8()?;
        let domain = if header & 0x80 == 0 {
            TimeDomain::Musical
        } else {
            TimeDomain::Absolute
        };
        let kind = header & 0x7F;
        let delta = cursor.read_vlq()?;
        let event_kind = match kind {
            0x00 => {
                let format_offset = cursor.position();
                let format = OscFormat::try_from(cursor.read_u8()?)
                    .map_err(|error| error_at(error, format_offset))?;
                let length = cursor.read_vlq_usize()?;
                EventKind::Osc(OscEvent {
                    format,
                    data: cursor.take(length)?.to_vec(),
                })
            }
            0x01 => {
                let status = cursor.read_u8()?;
                let data1 = cursor.read_u8()?;
                let data2 =
                    if flags & FLAG_FIXED_MIDI_WIDTH != 0 || !matches!(status >> 4, 0xC | 0xD) {
                        cursor.read_u8()?
                    } else {
                        0
                    };
                EventKind::Midi([status, data1, data2])
            }
            0x02 => {
                let type_id = cursor.read_u8()?;
                let length = cursor.read_vlq_usize()?;
                EventKind::Meta {
                    type_id,
                    data: cursor.take(length)?.to_vec(),
                }
            }
            0x03 => {
                let length = cursor.read_vlq_usize()?;
                let payload = cursor.take(length)?;
                let (status, data) = if flags & FLAG_SYSEX_STATUS_IN_PAYLOAD != 0 {
                    let (status, data) = payload
                        .split_first()
                        .ok_or_else(|| cursor.error("SysEx status is missing"))?;
                    (*status, data.to_vec())
                } else {
                    (0xF0, payload.to_vec())
                };
                EventKind::Sysex(SysexEvent { status, data })
            }
            0x7E => {
                let type_id = cursor.read_u8()?;
                let length = cursor.read_vlq_usize()?;
                EventKind::Custom {
                    type_id,
                    data: cursor.take(length)?.to_vec(),
                }
            }
            _ => return Err(cursor.error("unsupported track event kind")),
        };
        events.push(Event {
            delta,
            domain,
            kind: event_kind,
        });
    }
    Ok(Track { events })
}

fn encode_event(event: &Event, out: &mut Vec<u8>, sysex_status: bool) -> Result<(), Error> {
    let kind = match event.kind {
        EventKind::Osc(_) => 0x00,
        EventKind::Midi(_) => 0x01,
        EventKind::Meta { .. } => 0x02,
        EventKind::Sysex(_) => 0x03,
        EventKind::Custom { .. } => 0x7E,
    };
    out.push((encode_domain(event.domain) << 7) | kind);
    write_vlq(event.delta, out);
    match &event.kind {
        EventKind::Osc(osc) => {
            out.push(osc.format as u8);
            write_vlq(osc.data.len() as u64, out);
            out.extend_from_slice(&osc.data);
        }
        EventKind::Midi(bytes) => out.extend_from_slice(bytes),
        EventKind::Meta { type_id, data } => {
            out.push(*type_id);
            write_vlq(data.len() as u64, out);
            out.extend_from_slice(data);
        }
        EventKind::Sysex(sysex) => {
            let length = sysex.data.len() + usize::from(sysex_status);
            write_vlq(length as u64, out);
            if sysex_status {
                out.push(sysex.status);
            }
            out.extend_from_slice(&sysex.data);
        }
        EventKind::Custom { type_id, data } => {
            out.push(*type_id);
            write_vlq(data.len() as u64, out);
            out.extend_from_slice(data);
        }
    }
    Ok(())
}

fn validate_event(event: &Event) -> Result<(), Error> {
    match &event.kind {
        EventKind::Osc(osc) => {
            checked_u32_len(osc.data.len(), "OSC payload is too large")?;
            if osc.format == OscFormat::Raw {
                if !matches!(osc.data.first(), Some(b'/') | Some(b'#')) {
                    return Err(Error::Invalid("RAW OSC payload must start with '/' or '#'"));
                }
                if !osc.data.len().is_multiple_of(4) {
                    return Err(Error::Invalid("RAW OSC payload must be four-byte aligned"));
                }
            }
        }
        EventKind::Midi([status, data1, data2]) => {
            if !(0x80..=0xEF).contains(status) {
                return Err(Error::Invalid("MIDI status must be a channel message"));
            }
            if *data1 > 0x7F || *data2 > 0x7F {
                return Err(Error::Invalid("MIDI data bytes must be seven-bit values"));
            }
            if matches!(status >> 4, 0xC | 0xD) && *data2 != 0 {
                return Err(Error::Invalid(
                    "single-data-byte MIDI messages require zero padding",
                ));
            }
        }
        EventKind::Meta { data, .. } | EventKind::Custom { data, .. } => {
            checked_u32_len(data.len(), "event payload is too large")?;
        }
        EventKind::Sysex(sysex) => {
            if !matches!(sysex.status, 0xF0 | 0xF7) {
                return Err(Error::Invalid("SysEx status must be 0xF0 or 0xF7"));
            }
            checked_u32_len(sysex.data.len(), "SysEx payload is too large")?;
        }
    }
    Ok(())
}

fn interpolate_anchors(
    anchors: &[SyncAnchor],
    value: u64,
    tick_to_time: bool,
) -> Result<u64, Error> {
    if anchors.len() < 2 {
        return Err(Error::TimeMapping("at least two sync anchors are required"));
    }
    let first = anchors.first().expect("length checked");
    let last = anchors.last().expect("length checked");
    let (minimum, maximum) = if tick_to_time {
        (first.tick, last.tick)
    } else {
        (first.time, last.time)
    };
    if value < minimum || value > maximum {
        return Err(Error::TimeMapping(
            "position is outside the sync anchor range",
        ));
    }
    for pair in anchors.windows(2) {
        let (x0, x1, y0, y1) = if tick_to_time {
            (pair[0].tick, pair[1].tick, pair[0].time, pair[1].time)
        } else {
            (pair[0].time, pair[1].time, pair[0].tick, pair[1].tick)
        };
        if value == x0 {
            return Ok(y0);
        }
        if value <= x1 {
            let numerator = u128::from(value - x0) * u128::from(y1 - y0);
            let interpolated = u128::from(y0) + numerator / u128::from(x1 - x0);
            return u64::try_from(interpolated)
                .map_err(|_| Error::DataOverflow("interpolated time exceeds u64"));
        }
    }
    Ok(if tick_to_time { last.time } else { last.tick })
}

fn ensure_strictly_increasing<I>(values: I, message: &'static str) -> Result<(), Error>
where
    I: IntoIterator<Item = u64>,
{
    let mut previous = None;
    for value in values {
        if previous.is_some_and(|item| value <= item) {
            return Err(Error::Invalid(message));
        }
        previous = Some(value);
    }
    Ok(())
}

fn error_at(error: Error, offset: usize) -> Error {
    match error {
        Error::Invalid(message) => Error::InvalidAt { offset, message },
        other => other,
    }
}

fn is_known_chunk(id: [u8; 4]) -> bool {
    matches!(
        id,
        CHUNK_TRACK | CHUNK_TEMPO | CHUNK_SYNC | CHUNK_MARKER | CHUNK_SMPTE
    )
}

fn checked_u32_len(length: usize, message: &'static str) -> Result<u32, Error> {
    u32::try_from(length).map_err(|_| Error::DataOverflow(message))
}

fn push_chunk(out: &mut Vec<u8>, id: [u8; 4], payload: &[u8]) -> Result<(), Error> {
    let length = checked_u32_len(payload.len(), "chunk payload is too large")?;
    out.extend_from_slice(&id);
    out.extend_from_slice(&length.to_le_bytes());
    out.extend_from_slice(payload);
    Ok(())
}

fn encode_domain(domain: TimeDomain) -> u8 {
    match domain {
        TimeDomain::Musical => 0,
        TimeDomain::Absolute => 1,
    }
}

fn decode_domain(value: u8, cursor: &Cursor<'_>) -> Result<TimeDomain, Error> {
    match value {
        0 => Ok(TimeDomain::Musical),
        1 => Ok(TimeDomain::Absolute),
        _ => Err(cursor.error("unsupported time domain")),
    }
}

fn write_vlq(mut value: u64, out: &mut Vec<u8>) {
    let mut buffer = [0u8; 10];
    let mut index = buffer.len();
    index -= 1;
    buffer[index] = (value & 0x7F) as u8;
    value >>= 7;
    while value > 0 {
        index -= 1;
        buffer[index] = ((value & 0x7F) as u8) | 0x80;
        value >>= 7;
    }
    out.extend_from_slice(&buffer[index..]);
}

struct Cursor<'a> {
    data: &'a [u8],
    index: usize,
    base_offset: usize,
}

impl<'a> Cursor<'a> {
    const fn new(data: &'a [u8], base_offset: usize) -> Self {
        Self {
            data,
            index: 0,
            base_offset,
        }
    }

    fn is_empty(&self) -> bool {
        self.index == self.data.len()
    }

    fn position(&self) -> usize {
        self.base_offset + self.index
    }

    fn error(&self, message: &'static str) -> Error {
        self.error_at(self.position(), message)
    }

    fn error_at(&self, offset: usize, message: &'static str) -> Error {
        Error::InvalidAt { offset, message }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], Error> {
        let end = self
            .index
            .checked_add(length)
            .ok_or_else(|| self.error("length overflows platform limits"))?;
        if end > self.data.len() {
            return Err(self.error("payload exceeds remaining input"));
        }
        let result = &self.data[self.index..end];
        self.index = end;
        Ok(result)
    }

    fn read_u8(&mut self) -> Result<u8, Error> {
        Ok(self.take(1)?[0])
    }

    fn read_u16(&mut self) -> Result<u16, Error> {
        let bytes = self.take(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn read_u32(&mut self) -> Result<u32, Error> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_u64(&mut self) -> Result<u64, Error> {
        let bytes = self.take(8)?;
        Ok(u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    fn read_vlq(&mut self) -> Result<u64, Error> {
        let start = self.position();
        let mut value = 0u64;
        for index in 0..10 {
            let byte = self.read_u8()?;
            if value > (u64::MAX >> 7) {
                return Err(self.error_at(start, "VLQ exceeds u64"));
            }
            value = (value << 7) | u64::from(byte & 0x7F);
            if byte & 0x80 == 0 {
                return Ok(value);
            }
            if index == 9 {
                return Err(self.error_at(start, "VLQ exceeds ten bytes"));
            }
        }
        Err(self.error_at(start, "invalid VLQ"))
    }

    fn read_vlq_usize(&mut self) -> Result<usize, Error> {
        usize::try_from(self.read_vlq()?)
            .map_err(|_| self.error("VLQ length exceeds platform limits"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn raw_message() -> Vec<u8> {
        b"/a\0\0,i\0\0\0\0\0\x01".to_vec()
    }

    fn all_features_sequence() -> Sequence {
        Sequence {
            ppq: 960,
            absolute_unit: AbsoluteUnit::Microseconds,
            flags: FLAG_FIXED_MIDI_WIDTH,
            tracks: vec![Track {
                events: vec![
                    Event {
                        delta: 0,
                        domain: TimeDomain::Musical,
                        kind: EventKind::Midi([0x90, 60, 100]),
                    },
                    Event {
                        delta: 250_000,
                        domain: TimeDomain::Absolute,
                        kind: EventKind::Osc(OscEvent {
                            format: OscFormat::Raw,
                            data: raw_message(),
                        }),
                    },
                    Event {
                        delta: 120,
                        domain: TimeDomain::Musical,
                        kind: EventKind::Custom {
                            type_id: 9,
                            data: vec![1, 2, 3],
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
                    time: 500_000,
                },
            ],
            markers: vec![
                Marker {
                    domain: TimeDomain::Musical,
                    position: 0,
                    name: "Intro".into(),
                    class: 0,
                    color_rgba: None,
                },
                Marker {
                    domain: TimeDomain::Absolute,
                    position: 250_000,
                    name: "Cue".into(),
                    class: 0x20,
                    color_rgba: Some(0xFF00FF00),
                },
            ],
            smpte_timing: Some(SmpteTiming {
                fps: SmpteFps::Fps29Drop,
                subframes: 80,
            }),
            unknown_chunks: vec![UnknownChunk {
                id: *b"TEST",
                data: vec![0xAA, 0xBB],
            }],
        }
    }

    #[test]
    fn complete_model_roundtrips() {
        let sequence = all_features_sequence();
        let encoded = sequence.encode().expect("encode");
        let decoded = Sequence::decode(&encoded).expect("decode");
        assert_eq!(decoded, sequence);
    }

    #[test]
    fn mixed_domain_deltas_are_independent() {
        let sequence = all_features_sequence();
        let track = &sequence.tracks[0];
        let mut musical = 0;
        let mut absolute = 0;
        for event in &track.events {
            match event.domain {
                TimeDomain::Musical => musical += event.delta,
                TimeDomain::Absolute => absolute += event.delta,
            }
        }
        assert_eq!(musical, 120);
        assert_eq!(absolute, 250_000);
    }

    #[test]
    fn sync_interpolation_is_checked() {
        let sequence = all_features_sequence();
        assert_eq!(sequence.tick_to_absolute(480).unwrap(), 250_000);
        assert_eq!(sequence.absolute_to_tick(250_000).unwrap(), 480);
        assert!(sequence.tick_to_absolute(961).is_err());
    }

    #[test]
    fn malformed_chunk_reports_offset() {
        let mut bytes = all_features_sequence().encode().unwrap();
        bytes.truncate(bytes.len() - 1);
        assert!(matches!(
            Sequence::decode(&bytes),
            Err(Error::InvalidAt { .. })
        ));
    }

    #[test]
    fn invalid_header_enum_reports_exact_offset() {
        let mut bytes = all_features_sequence().encode().unwrap();
        bytes[8] = 0xFF;
        assert!(matches!(
            Sequence::decode(&bytes),
            Err(Error::InvalidAt { offset: 8, .. })
        ));
    }

    #[test]
    fn malformed_vlq_is_rejected_without_panicking() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"TSQ1");
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&480u16.to_le_bytes());
        bytes.extend_from_slice(&[0, 0]);
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&FLAG_FIXED_MIDI_WIDTH.to_le_bytes());
        bytes.extend_from_slice(b"TRK ");
        bytes.extend_from_slice(&11u32.to_le_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&[0x80; 10]);
        assert!(matches!(
            Sequence::decode(&bytes),
            Err(Error::InvalidAt { offset: 23, .. })
        ));
    }

    #[test]
    fn non_monotonic_sync_anchors_are_rejected() {
        let mut sequence = all_features_sequence();
        sequence.sync_anchors.push(SyncAnchor {
            tick: 480,
            time: 750_000,
        });
        assert!(matches!(sequence.validate(), Err(Error::Invalid(_))));
    }
}
