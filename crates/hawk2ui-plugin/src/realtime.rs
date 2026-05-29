//! Realtime visual data transport records.

use rtrb::{Consumer, Producer, PushError, RingBuffer};
use serde::{Deserialize, Serialize};

/// Maximum number of `f32` samples carried inline by a single [`RealtimeVisualPacket`].
///
/// Packets store their channel id and samples in fixed inline buffers so that
/// constructing, moving, and dropping a packet performs **no heap allocation or
/// deallocation** — the hard requirement for [`RealtimeVisualAudioWriter::audio_thread_push`],
/// which runs on the realtime audio thread. Payloads longer than this are clamped to
/// their leading `MAX_VISUAL_SAMPLES` values: a spectrum or scope frame degrades
/// gracefully rather than being rejected (and made invisible) on the hot path.
///
/// A packet is therefore roughly `MAX_VISUAL_SAMPLES * 4` bytes, and a transport
/// preallocates `capacity` packets — so a 4096-sample bound at capacity 64 reserves
/// about 1 MiB. Raise this if an analyzer needs more bins; the only cost is that
/// preallocated footprint.
pub const MAX_VISUAL_SAMPLES: usize = 4096;

/// Maximum byte length of a channel identifier carried inline by a [`RealtimeVisualPacket`].
///
/// Channel ids are short labels (`"out"`, `"fft"`, `"scope.left"`); a longer id is
/// truncated on a UTF-8 character boundary.
pub const MAX_CHANNEL_ID_LEN: usize = 32;

/// Realtime visual channel kind.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RealtimeChannelKind {
    /// Meter sample stream.
    Meter,
    /// Analyzer bin stream.
    Analyzer,
    /// Oscilloscope sample stream.
    Scope,
    /// Modulation source stream.
    Modulation,
}

/// Frame drop policy for full visual transport buffers.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum FrameDropPolicy {
    /// Drop the oldest frame and accept the new frame when the consumer endpoint is available.
    DropOldest,
    /// Drop the newest frame and keep the existing buffer.
    DropNewest,
}

/// Realtime visual packet.
///
/// Carries its channel id and samples in fixed inline buffers, so the packet owns no
/// heap: constructing it, moving it through the transport ring, and dropping it never
/// allocate or free. That is what makes pushing one from the realtime audio thread
/// safe. The invariant is proven structurally by the `realtime_visual_packet_owns_no_heap`
/// test (`!std::mem::needs_drop`); it breaks the build the moment a heap-owning field
/// (`Vec`/`String`/`Box`) is reintroduced. Inputs larger than [`MAX_VISUAL_SAMPLES`] /
/// [`MAX_CHANNEL_ID_LEN`] are clamped on construction rather than allocating.
#[derive(Clone, Debug, PartialEq)]
pub struct RealtimeVisualPacket {
    /// Channel kind.
    pub kind: RealtimeChannelKind,
    channel_id: [u8; MAX_CHANNEL_ID_LEN],
    channel_id_len: usize,
    values: [f32; MAX_VISUAL_SAMPLES],
    values_len: usize,
}

impl RealtimeVisualPacket {
    /// Creates a meter packet.
    #[must_use]
    pub fn meter(channel_id: &str, value: f32) -> Self {
        Self::with_samples(RealtimeChannelKind::Meter, channel_id, &[value])
    }

    /// Creates an analyzer packet from a bin slice (clamped to [`MAX_VISUAL_SAMPLES`]).
    #[must_use]
    pub fn analyzer(channel_id: &str, bins: &[f32]) -> Self {
        Self::with_samples(RealtimeChannelKind::Analyzer, channel_id, bins)
    }

    /// Creates a scope packet from a sample slice (clamped to [`MAX_VISUAL_SAMPLES`]).
    #[must_use]
    pub fn scope(channel_id: &str, samples: &[f32]) -> Self {
        Self::with_samples(RealtimeChannelKind::Scope, channel_id, samples)
    }

    /// Creates a modulation packet.
    #[must_use]
    pub fn modulation(channel_id: &str, value: f32) -> Self {
        Self::with_samples(RealtimeChannelKind::Modulation, channel_id, &[value])
    }

    /// Returns the channel identifier.
    #[must_use]
    pub fn channel_id(&self) -> &str {
        std::str::from_utf8(&self.channel_id[..self.channel_id_len]).unwrap_or_default()
    }

    /// Returns the visual sample payload.
    #[must_use]
    pub fn values(&self) -> &[f32] {
        &self.values[..self.values_len]
    }

    /// Builds a packet by copying `channel_id` and `samples` into inline buffers.
    ///
    /// Performs no heap allocation: an oversized `channel_id` is truncated on a UTF-8
    /// boundary and an oversized `samples` slice is clamped to the leading
    /// [`MAX_VISUAL_SAMPLES`] values. Clamping is silent and non-panicking by design —
    /// this runs on the realtime audio thread, so an over-capacity payload degrades the
    /// frame rather than aborting; producers should size payloads to [`MAX_VISUAL_SAMPLES`].
    fn with_samples(kind: RealtimeChannelKind, channel_id: &str, samples: &[f32]) -> Self {
        let mut channel_id_buffer = [0_u8; MAX_CHANNEL_ID_LEN];
        let channel_id_len = copy_channel_id(&mut channel_id_buffer, channel_id);
        let mut values = [0.0_f32; MAX_VISUAL_SAMPLES];
        let values_len = samples.len().min(MAX_VISUAL_SAMPLES);
        values[..values_len].copy_from_slice(&samples[..values_len]);
        Self {
            kind,
            channel_id: channel_id_buffer,
            channel_id_len,
            values,
            values_len,
        }
    }
}

/// Copies `channel_id` into `buffer`, truncating on a UTF-8 boundary, and returns the
/// byte length written.
fn copy_channel_id(buffer: &mut [u8; MAX_CHANNEL_ID_LEN], channel_id: &str) -> usize {
    let end = channel_id.floor_char_boundary(MAX_CHANNEL_ID_LEN);
    buffer[..end].copy_from_slice(&channel_id.as_bytes()[..end]);
    end
}

/// Result of an audio-thread visual packet write.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RealtimePushResult {
    /// Whether the submitted packet was accepted.
    pub accepted: bool,
    /// Frames dropped to complete the write.
    pub dropped_frames: usize,
}

/// Validation error for realtime visual frame-rate gates.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RealtimeVisualFrameRateError {
    /// Stable validation code.
    pub code: String,
    /// Human-readable message.
    pub message: String,
}

impl RealtimeVisualFrameRateError {
    fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

/// UI-side frame gate for reducing realtime visual drain cadence.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RealtimeVisualFrameGate {
    target_hz: u16,
    minimum_interval_ms: u64,
    last_presented_ms: Option<u64>,
}

impl RealtimeVisualFrameGate {
    /// Maximum accepted realtime visual presentation rate.
    pub const MAX_TARGET_HZ: u16 = 240;

    /// Creates a validated frame gate.
    ///
    /// # Errors
    ///
    /// Returns [`RealtimeVisualFrameRateError`] when `target_hz` is zero or above
    /// [`Self::MAX_TARGET_HZ`].
    pub fn new(target_hz: u16) -> Result<Self, RealtimeVisualFrameRateError> {
        if target_hz == 0 || target_hz > Self::MAX_TARGET_HZ {
            return Err(RealtimeVisualFrameRateError::new(
                "realtime.frame-rate.invalid",
                format!(
                    "realtime visual target_hz must be between 1 and {}",
                    Self::MAX_TARGET_HZ
                ),
            ));
        }

        Ok(Self {
            target_hz,
            minimum_interval_ms: ceil_milliseconds_per_frame(target_hz),
            last_presented_ms: None,
        })
    }

    /// Returns the target presentation rate in Hz.
    #[must_use]
    pub const fn target_hz(&self) -> u16 {
        self.target_hz
    }

    /// Returns the minimum interval between presented visual frames.
    #[must_use]
    pub const fn minimum_interval_ms(&self) -> u64 {
        self.minimum_interval_ms
    }

    /// Returns the last accepted presentation timestamp.
    #[must_use]
    pub const fn last_presented_ms(&self) -> Option<u64> {
        self.last_presented_ms
    }

    /// Returns whether a UI drain should present at `timestamp_ms` and updates the gate state.
    pub fn should_present_at(&mut self, timestamp_ms: u64) -> bool {
        match self.last_presented_ms {
            None => {
                self.last_presented_ms = Some(timestamp_ms);
                true
            }
            Some(last_presented_ms)
                if timestamp_ms >= last_presented_ms.saturating_add(self.minimum_interval_ms) =>
            {
                self.last_presented_ms = Some(timestamp_ms);
                true
            }
            Some(_) => false,
        }
    }
}

/// Audio-thread writer for preallocated non-blocking realtime visual data.
#[derive(Debug)]
pub struct RealtimeVisualAudioWriter {
    capacity: usize,
    drop_policy: FrameDropPolicy,
    producer: Producer<RealtimeVisualPacket>,
}

impl RealtimeVisualAudioWriter {
    /// Writes a packet from the audio thread without blocking or allocating.
    ///
    /// The packet owns no heap (see [`RealtimeVisualPacket`]), so moving it into the
    /// preallocated ring — and dropping a rejected packet — never allocates or frees.
    /// Split writers cannot remove old packets because they do not own the UI reader
    /// endpoint, so a full buffer degrades to dropping the newest packet and reports it.
    #[must_use]
    pub fn audio_thread_push(&mut self, packet: RealtimeVisualPacket) -> RealtimePushResult {
        push_without_waiting(&mut self.producer, packet, self.drop_policy)
    }

    /// Returns preallocated capacity.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }
}

/// UI-thread reader for preallocated non-blocking realtime visual data.
#[derive(Debug)]
pub struct RealtimeVisualUiReader {
    capacity: usize,
    consumer: Consumer<RealtimeVisualPacket>,
}

impl RealtimeVisualUiReader {
    /// Drains packets on the UI thread.
    pub fn ui_drain(&mut self) -> Vec<RealtimeVisualPacket> {
        drain_consumer(&mut self.consumer)
    }

    /// Drains packets only when the provided frame gate allows a UI presentation.
    pub fn ui_drain_due(
        &mut self,
        timestamp_ms: u64,
        frame_gate: &mut RealtimeVisualFrameGate,
    ) -> Option<Vec<RealtimeVisualPacket>> {
        frame_gate
            .should_present_at(timestamp_ms)
            .then(|| self.ui_drain())
    }

    /// Returns preallocated capacity.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns pending packet count.
    #[must_use]
    pub fn pending_len(&self) -> usize {
        self.consumer.slots()
    }
}

/// Preallocated non-blocking realtime visual data transport.
#[derive(Debug)]
pub struct RealtimeVisualTransport {
    capacity: usize,
    drop_policy: FrameDropPolicy,
    producer: Producer<RealtimeVisualPacket>,
    consumer: Consumer<RealtimeVisualPacket>,
}

impl RealtimeVisualTransport {
    /// Creates a preallocated single-owner visual transport.
    #[must_use]
    pub fn preallocated(capacity: usize, drop_policy: FrameDropPolicy) -> Self {
        let (producer, consumer) = RingBuffer::new(capacity);
        Self {
            capacity,
            drop_policy,
            producer,
            consumer,
        }
    }

    /// Creates split realtime endpoints for audio-thread writes and UI-thread reads.
    #[must_use]
    pub fn split_preallocated(
        capacity: usize,
        drop_policy: FrameDropPolicy,
    ) -> (RealtimeVisualAudioWriter, RealtimeVisualUiReader) {
        let (producer, consumer) = RingBuffer::new(capacity);
        (
            RealtimeVisualAudioWriter {
                capacity,
                drop_policy,
                producer,
            },
            RealtimeVisualUiReader { capacity, consumer },
        )
    }

    /// Writes a packet from the audio thread without blocking or allocating.
    #[must_use]
    pub fn audio_thread_push(&mut self, packet: RealtimeVisualPacket) -> RealtimePushResult {
        match self.producer.push(packet) {
            Ok(()) => accepted_without_drop(),
            Err(PushError::Full(packet)) => match self.drop_policy {
                FrameDropPolicy::DropOldest => {
                    let _ = self.consumer.pop();
                    match self.producer.push(packet) {
                        Ok(()) => accepted_with_drop(),
                        Err(PushError::Full(_)) => rejected_with_drop(),
                    }
                }
                FrameDropPolicy::DropNewest => rejected_with_drop(),
            },
        }
    }

    /// Drains packets on the UI thread.
    pub fn ui_drain(&mut self) -> Vec<RealtimeVisualPacket> {
        drain_consumer(&mut self.consumer)
    }

    /// Drains packets only when the provided frame gate allows a UI presentation.
    pub fn ui_drain_due(
        &mut self,
        timestamp_ms: u64,
        frame_gate: &mut RealtimeVisualFrameGate,
    ) -> Option<Vec<RealtimeVisualPacket>> {
        frame_gate
            .should_present_at(timestamp_ms)
            .then(|| self.ui_drain())
    }

    /// Returns preallocated capacity.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns pending packet count.
    #[must_use]
    pub fn pending_len(&self) -> usize {
        self.consumer.slots()
    }
}

fn push_without_waiting(
    producer: &mut Producer<RealtimeVisualPacket>,
    packet: RealtimeVisualPacket,
    drop_policy: FrameDropPolicy,
) -> RealtimePushResult {
    match producer.push(packet) {
        Ok(()) => accepted_without_drop(),
        Err(PushError::Full(_)) => match drop_policy {
            FrameDropPolicy::DropOldest | FrameDropPolicy::DropNewest => rejected_with_drop(),
        },
    }
}

fn drain_consumer(consumer: &mut Consumer<RealtimeVisualPacket>) -> Vec<RealtimeVisualPacket> {
    let mut packets = Vec::with_capacity(consumer.slots());
    while let Ok(packet) = consumer.pop() {
        packets.push(packet);
    }
    packets
}

const fn ceil_milliseconds_per_frame(target_hz: u16) -> u64 {
    let target_hz = target_hz as u64;
    1000_u64.div_ceil(target_hz)
}

const fn accepted_without_drop() -> RealtimePushResult {
    RealtimePushResult {
        accepted: true,
        dropped_frames: 0,
    }
}

const fn accepted_with_drop() -> RealtimePushResult {
    RealtimePushResult {
        accepted: true,
        dropped_frames: 1,
    }
}

const fn rejected_with_drop() -> RealtimePushResult {
    RealtimePushResult {
        accepted: false,
        dropped_frames: 1,
    }
}
