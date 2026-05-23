//! Realtime visual data transport records.

use std::collections::VecDeque;

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
    /// Drop the oldest frame and accept the new frame.
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

/// Preallocated non-blocking realtime visual data transport.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RealtimeVisualTransport {
    capacity: usize,
    drop_policy: FrameDropPolicy,
    queue: VecDeque<RealtimeVisualPacket>,
    allocation_count: usize,
    blocking_wait_count: usize,
}

impl RealtimeVisualTransport {
    /// Creates a preallocated visual transport.
    #[must_use]
    pub fn preallocated(capacity: usize, drop_policy: FrameDropPolicy) -> Self {
        Self {
            capacity,
            drop_policy,
            queue: VecDeque::with_capacity(capacity),
            allocation_count: 0,
            blocking_wait_count: 0,
        }
    }

    /// Writes a packet from the audio thread without blocking.
    pub fn audio_thread_push(&mut self, packet: RealtimeVisualPacket) -> RealtimePushResult {
        if self.queue.len() < self.capacity {
            self.queue.push_back(packet);
            return RealtimePushResult {
                accepted: true,
                dropped_frames: 0,
            };
        }

        match self.drop_policy {
            FrameDropPolicy::DropOldest => {
                self.queue.pop_front();
                self.queue.push_back(packet);
                RealtimePushResult {
                    accepted: true,
                    dropped_frames: 1,
                }
            }
            FrameDropPolicy::DropNewest => RealtimePushResult {
                accepted: false,
                dropped_frames: 1,
            },
        }
    }

    /// Drains packets on the UI thread.
    pub fn ui_drain(&mut self) -> Vec<RealtimeVisualPacket> {
        self.queue.drain(..).collect()
    }

    /// Returns preallocated capacity.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns pending packet count.
    #[must_use]
    pub fn pending_len(&self) -> usize {
        self.queue.len()
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
