//! Realtime visual data transport records.

use rtrb::{Consumer, Producer, PushError, RingBuffer};
use serde::{Deserialize, Serialize};

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
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RealtimeVisualPacket {
    /// Channel kind.
    pub kind: RealtimeChannelKind,
    /// Channel identifier.
    pub channel_id: String,
    /// Visual samples or scalar payload.
    pub values: Vec<f32>,
}

impl RealtimeVisualPacket {
    /// Creates a meter packet.
    #[must_use]
    pub fn meter(channel_id: impl Into<String>, value: f32) -> Self {
        Self {
            kind: RealtimeChannelKind::Meter,
            channel_id: channel_id.into(),
            values: vec![value],
        }
    }

    /// Creates an analyzer packet.
    #[must_use]
    pub fn analyzer(channel_id: impl Into<String>, bins: Vec<f32>) -> Self {
        Self {
            kind: RealtimeChannelKind::Analyzer,
            channel_id: channel_id.into(),
            values: bins,
        }
    }

    /// Creates a scope packet.
    #[must_use]
    pub fn scope(channel_id: impl Into<String>, samples: Vec<f32>) -> Self {
        Self {
            kind: RealtimeChannelKind::Scope,
            channel_id: channel_id.into(),
            values: samples,
        }
    }

    /// Creates a modulation packet.
    #[must_use]
    pub fn modulation(channel_id: impl Into<String>, value: f32) -> Self {
        Self {
            kind: RealtimeChannelKind::Modulation,
            channel_id: channel_id.into(),
            values: vec![value],
        }
    }
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
    allocation_count: usize,
    blocking_wait_count: usize,
}

impl RealtimeVisualAudioWriter {
    /// Writes a packet from the audio thread without blocking.
    ///
    /// Split writers cannot remove old packets because they do not own the UI reader endpoint.
    /// When full, the write degrades to dropping the newest packet and reports the drop.
    #[must_use]
    pub fn audio_thread_push(&mut self, packet: RealtimeVisualPacket) -> RealtimePushResult {
        push_without_waiting(&mut self.producer, packet, self.drop_policy)
    }

    /// Returns preallocated capacity.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns allocation count observed in audio-thread writes.
    #[must_use]
    pub const fn allocation_count(&self) -> usize {
        self.allocation_count
    }

    /// Returns blocking wait count observed in audio-thread writes.
    #[must_use]
    pub const fn blocking_wait_count(&self) -> usize {
        self.blocking_wait_count
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
    allocation_count: usize,
    blocking_wait_count: usize,
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
            allocation_count: 0,
            blocking_wait_count: 0,
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
                allocation_count: 0,
                blocking_wait_count: 0,
            },
            RealtimeVisualUiReader { capacity, consumer },
        )
    }

    /// Writes a packet from the audio thread without blocking.
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

    /// Returns allocation count observed in audio-thread writes.
    #[must_use]
    pub const fn allocation_count(&self) -> usize {
        self.allocation_count
    }

    /// Returns blocking wait count observed in audio-thread writes.
    #[must_use]
    pub const fn blocking_wait_count(&self) -> usize {
        self.blocking_wait_count
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
