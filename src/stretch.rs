//! Changes how fast audio plays without changing its pitch.
//!
//! This is the WSOLA method: the output is built from short pieces of the
//! input, taken closer together to go faster or further apart to go slower,
//! and each piece is nudged to where it lines up best with the one before it
//! so the joins do not buzz. At normal speed every piece lines up exactly and
//! the audio passes through unchanged.

use rodio::source::SeekError;
use rodio::{ChannelCount, SampleRate, Source};
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::Duration;

pub struct Stretch<S> {
    inner: S,
    channels: usize,
    rate: u32,
    /// The speed, as the bits of an `f32`. 1.0 is normal.
    speed: Arc<AtomicU32>,
    /// Where in the audio playback has reached, in milliseconds. Output time
    /// is no guide to this once the speed is not 1.
    position_ms: Arc<AtomicU64>,
    /// Input still needed, interleaved. Its first frame is frame `base` of
    /// the audio.
    input: Vec<f32>,
    base: u64,
    /// Where the next piece should start, in frames from the start of `input`.
    next: f64,
    /// Where the input carries straight on from the last piece, in the same
    /// terms as `next`.
    natural: i64,
    /// The input that naturally follows the last piece, to blend into the next.
    tail: Vec<f32>,
    out: VecDeque<f32>,
    input_ended: bool,
    done: bool,
    /// Sizes in frames: how far each piece advances the output, how much of
    /// it is blended with the last, and how far a piece may be nudged.
    hop: usize,
    overlap: usize,
    search: usize,
}

impl<S: Source> Stretch<S> {
    pub fn new(inner: S, speed: Arc<AtomicU32>, position_ms: Arc<AtomicU64>) -> Self {
        let rate = inner.sample_rate().get();
        let milliseconds = |ms: u32| (rate * ms / 1000).max(1) as usize;
        position_ms.store(0, Ordering::Relaxed);
        Self {
            channels: usize::from(inner.channels().get()),
            rate,
            inner,
            speed,
            position_ms,
            input: Vec::new(),
            base: 0,
            next: 0.0,
            natural: 0,
            tail: Vec::new(),
            out: VecDeque::new(),
            input_ended: false,
            done: false,
            hop: milliseconds(20),
            overlap: milliseconds(10),
            // Long enough to cover one cycle of a low voice.
            search: milliseconds(12),
        }
    }

    fn frames(&self) -> usize {
        self.input.len() / self.channels
    }

    /// Reads input until `frames` of it are held, or it runs out.
    fn pull(&mut self, frames: usize) {
        while !self.input_ended && self.frames() < frames {
            for _ in 0..self.channels {
                match self.inner.next() {
                    Some(sample) => self.input.push(sample),
                    None => {
                        self.input_ended = true;
                        // Never keep part of a frame.
                        self.input.truncate(self.frames() * self.channels);
                        break;
                    }
                }
            }
        }
    }

    /// Every other frame from `start`, with its channels added together.
    fn mono(&self, samples: &[f32], start: usize, frames: usize) -> Vec<f32> {
        (0..frames)
            .step_by(2)
            .map(|frame| {
                let at = (start + frame) * self.channels;
                samples[at..at + self.channels].iter().sum()
            })
            .collect()
    }

    /// The start near `nominal` whose opening best matches `tail`.
    ///
    /// Only every other frame is compared and only every other start is
    /// tried, which is four times less work for no audible difference. This
    /// runs on the audio thread, so it has to be quick.
    fn best_start(&self, nominal: usize) -> usize {
        let last = self.frames().saturating_sub(self.overlap);
        let from = nominal.saturating_sub(self.search);
        let to = (nominal + self.search).min(last);
        if from >= to {
            return nominal.min(last);
        }
        let wanted = self.mono(&self.tail, 0, self.overlap);
        let region = self.mono(&self.input, from, to - from + self.overlap);
        let mut best = (nominal.min(last), f32::MIN);
        for step in 0..=(to - from) / 2 {
            let candidate = &region[step..step + wanted.len()];
            let (mut alike, mut energy) = (0.0f32, 1e-9f32);
            for (a, b) in wanted.iter().zip(candidate) {
                alike += a * b;
                energy += b * b;
            }
            let score = alike / energy.sqrt();
            if score > best.1 {
                best = (from + step * 2, score);
            }
        }
        best.0
    }

    /// Produces the next piece of output.
    fn fill(&mut self) {
        let speed = f32::from_bits(self.speed.load(Ordering::Relaxed)).clamp(0.25, 4.0);
        let nominal = self.next.round().max(0.0) as usize;
        self.pull(nominal + self.search + self.hop + self.overlap);
        let frames = self.frames();
        if frames <= nominal {
            self.out.extend(self.tail.drain(..));
            self.done = true;
            return;
        }
        // When the piece is due exactly where the last one left off, as it
        // always is at normal speed, it already lines up.
        let start = if self.tail.is_empty() || self.natural == nominal as i64 {
            nominal
        } else {
            self.best_start(nominal)
        };
        let piece = (self.hop + self.overlap).min(frames - start);
        let blended = self.overlap.min(piece);
        let channels = self.channels;
        for frame in 0..blended {
            let fade = (frame as f32 + 0.5) / self.overlap as f32;
            for channel in 0..channels {
                let new = self.input[(start + frame) * channels + channel];
                let sample = match self.tail.get(frame * channels + channel) {
                    Some(old) => old * (1.0 - fade) + new * fade,
                    None => new,
                };
                self.out.push_back(sample);
            }
        }
        let whole = piece == self.hop + self.overlap;
        let plain_end = if whole { self.hop } else { piece };
        self.out
            .extend(&self.input[(start + blended) * channels..(start + plain_end) * channels]);
        self.tail.clear();
        if whole {
            self.tail
                .extend(&self.input[(start + self.hop) * channels..(start + piece) * channels]);
        } else {
            self.done = true;
        }
        let position = self.base + start as u64;
        self.position_ms
            .store(position * 1000 / u64::from(self.rate), Ordering::Relaxed);

        // The pieces are due at even steps through the input, wherever the
        // last one was nudged to; stepping on from the nudged start instead
        // would let the nudges add up and change the speed.
        self.natural = (start + self.hop) as i64;
        self.next = if speed == 1.0 {
            self.natural as f64
        } else {
            self.next + self.hop as f64 * f64::from(speed)
        };
        // Input before anything the next piece could use is finished with.
        let finished = (self.next.floor() as usize)
            .saturating_sub(self.search)
            .min(frames);
        self.input.drain(..finished * channels);
        self.base += finished as u64;
        self.next -= finished as f64;
        self.natural -= finished as i64;
    }
}

impl<S: Source> Iterator for Stretch<S> {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        while self.out.is_empty() && !self.done {
            self.fill();
        }
        self.out.pop_front()
    }
}

impl<S: Source> Source for Stretch<S> {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        self.inner.channels()
    }

    fn sample_rate(&self) -> SampleRate {
        self.inner.sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }

    fn try_seek(&mut self, to: Duration) -> Result<(), SeekError> {
        self.inner.try_seek(to)?;
        self.input.clear();
        self.tail.clear();
        self.out.clear();
        self.base = (to.as_secs_f64() * f64::from(self.rate)) as u64;
        self.next = 0.0;
        self.natural = 0;
        self.input_ended = false;
        self.done = false;
        self.position_ms
            .store(to.as_millis() as u64, Ordering::Relaxed);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rodio::buffer::SamplesBuffer;
    use std::num::NonZero;

    const RATE: u32 = 48_000;

    fn stretched(speed: f32, channels: u16) -> (Vec<f32>, Vec<f32>, u64) {
        let frames = RATE as usize * 2;
        let input: Vec<f32> = (0..frames * usize::from(channels))
            .map(|i| ((i / usize::from(channels)) as f32 * 0.03).sin() * 0.5)
            .collect();
        let source = SamplesBuffer::new(
            NonZero::new(channels).unwrap(),
            NonZero::new(RATE).unwrap(),
            input.clone(),
        );
        let position = Arc::new(AtomicU64::new(0));
        let speed = Arc::new(AtomicU32::new(speed.to_bits()));
        let output: Vec<f32> = Stretch::new(source, speed, position.clone()).collect();
        (input, output, position.load(Ordering::Relaxed))
    }

    #[test]
    fn normal_speed_passes_audio_through_unchanged() {
        let (input, output, _) = stretched(1.0, 2);
        assert_eq!(input.len(), output.len());
        let worst = input
            .iter()
            .zip(&output)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f32::max);
        assert!(worst < 1e-5, "differs by {worst}");
    }

    #[test]
    fn length_follows_speed_and_position_follows_the_audio() {
        for (speed, channels) in [(2.0, 1), (0.5, 2), (1.25, 2)] {
            let (input, output, position_ms) = stretched(speed, channels);
            let expected = input.len() as f32 / speed;
            let ratio = output.len() as f32 / expected;
            assert!(
                (0.97..1.03).contains(&ratio),
                "speed {speed}: ratio {ratio}"
            );
            // The two seconds of input were all played, whatever the speed.
            assert!(
                (1900..=2000).contains(&position_ms),
                "position {position_ms}"
            );
            // The joins must not make it louder than the input ever was.
            assert!(output.iter().all(|sample| sample.abs() <= 0.51));
        }
    }
}
