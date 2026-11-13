//! Records the default microphone to a temporary WAV file.
//!
//! WAV keeps this simple and AudioPub converts whatever it is given. It is
//! large, though: a mono recording at 48 kHz is about 5.5 MB a minute, so the
//! site's 500 MB limit is roughly an hour and a half.

use rodio::Source;
use rodio::microphone::MicrophoneBuilder;
use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;
use tempfile::NamedTempFile;

pub struct Recorder {
    file: NamedTempFile,
    stop: StopOnDrop,
    paused: Arc<AtomicBool>,
    /// Frames written, one per sample per channel.
    frames: Arc<AtomicU64>,
    rate: u32,
    thread: JoinHandle<Result<(), String>>,
}

/// A finished recording.
pub struct Recording {
    pub file: NamedTempFile,
    pub length: Duration,
}

impl Recorder {
    /// Starts recording. Returns once the microphone is open.
    pub fn start() -> Result<Self, String> {
        let file = tempfile::Builder::new()
            .prefix("pubseed-recording-")
            .suffix(".wav")
            .tempfile()
            .map_err(|e| format!("Couldn't create a file for the recording: {e}"))?;
        let output = file
            .reopen()
            .map_err(|e| format!("Couldn't open the recording file: {e}"))?;
        let stop = Arc::new(AtomicBool::new(false));
        let paused = Arc::new(AtomicBool::new(false));
        let frames = Arc::new(AtomicU64::new(0));
        let (opened_tx, opened_rx) = std::sync::mpsc::channel();
        let thread = {
            let (stop, paused, frames) = (stop.clone(), paused.clone(), frames.clone());
            // The microphone has to stay on the thread that opened it.
            std::thread::spawn(move || {
                let microphone = MicrophoneBuilder::new()
                    .default_device()
                    .and_then(|builder| builder.default_config())
                    .map_err(|e| format!("No microphone was found: {e}"))
                    .and_then(|builder| {
                        builder
                            .open_stream()
                            .map_err(|e| format!("Couldn't open the microphone: {e}"))
                    });
                let microphone = match microphone {
                    Ok(microphone) => microphone,
                    Err(error) => {
                        let _ = opened_tx.send(Err(error.clone()));
                        return Err(error);
                    }
                };
                let channels = microphone.channels().get();
                let rate = microphone.sample_rate().get();
                let _ = opened_tx.send(Ok(rate));
                write_wav(microphone, output, channels, rate, &stop, &paused, &frames)
                    .map_err(|e| format!("The recording couldn't be saved: {e}"))
            })
        };
        let rate = opened_rx
            .recv()
            .map_err(|_| "The microphone stopped unexpectedly".to_string())??;
        Ok(Self {
            file,
            stop: StopOnDrop(stop),
            paused,
            frames,
            rate,
            thread,
        })
    }

    pub fn length(&self) -> Duration {
        Duration::from_secs_f64(self.frames.load(Ordering::Relaxed) as f64 / f64::from(self.rate))
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }

    pub fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::Relaxed);
    }

    /// Stops recording and finishes the file.
    pub fn finish(self) -> Result<Recording, String> {
        self.stop.0.store(true, Ordering::Relaxed);
        let length = self.length();
        match self.thread.join() {
            Ok(Ok(())) => Ok(Recording {
                file: self.file,
                length,
            }),
            Ok(Err(error)) => Err(error),
            Err(_) => Err("The recording stopped unexpectedly".into()),
        }
    }
}

/// Tells the recording thread to stop when the recorder goes away, so a
/// recorder dropped without `finish` never leaves the microphone open.
struct StopOnDrop(Arc<AtomicBool>);

impl Drop for StopOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

fn write_wav(
    microphone: impl Iterator<Item = f32>,
    output: File,
    channels: u16,
    rate: u32,
    stop: &AtomicBool,
    paused: &AtomicBool,
    frames: &AtomicU64,
) -> std::io::Result<()> {
    let mut out = BufWriter::new(output);
    write_header(&mut out, channels, rate, 0)?;
    let mut bytes = 0u32;
    let mut in_frame = 0u16;
    for sample in microphone {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        // While paused the microphone is still read, so nothing builds up,
        // but only whole frames are skipped so the channels stay in order.
        if paused.load(Ordering::Relaxed) && in_frame == 0 {
            continue;
        }
        let value = (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16;
        out.write_all(&value.to_le_bytes())?;
        bytes = bytes.saturating_add(2);
        in_frame += 1;
        if in_frame == channels {
            in_frame = 0;
            frames.fetch_add(1, Ordering::Relaxed);
        }
    }
    // Now the sizes are known, the header can say them.
    out.seek(SeekFrom::Start(0))?;
    write_header(&mut out, channels, rate, bytes)?;
    out.flush()
}

fn write_header(
    out: &mut impl Write,
    channels: u16,
    rate: u32,
    data_bytes: u32,
) -> std::io::Result<()> {
    let block_align = channels * 2;
    out.write_all(b"RIFF")?;
    out.write_all(&(36 + data_bytes).to_le_bytes())?;
    out.write_all(b"WAVEfmt ")?;
    out.write_all(&16u32.to_le_bytes())?;
    out.write_all(&1u16.to_le_bytes())?; // PCM
    out.write_all(&channels.to_le_bytes())?;
    out.write_all(&rate.to_le_bytes())?;
    out.write_all(&(rate * u32::from(block_align)).to_le_bytes())?;
    out.write_all(&block_align.to_le_bytes())?;
    out.write_all(&16u16.to_le_bytes())?; // bits per sample
    out.write_all(b"data")?;
    out.write_all(&data_bytes.to_le_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_a_wav_that_decodes() {
        let file = NamedTempFile::new().unwrap();
        let samples = (0..48_000).map(|i| (i as f32 / 20.0).sin() * 0.5);
        let frames = AtomicU64::new(0);
        let stop = AtomicBool::new(false);
        let paused = AtomicBool::new(false);
        write_wav(
            samples,
            file.reopen().unwrap(),
            2,
            24_000,
            &stop,
            &paused,
            &frames,
        )
        .unwrap();
        assert_eq!(frames.load(Ordering::Relaxed), 24_000);
        let decoded = rodio::Decoder::try_from(file.reopen().unwrap()).unwrap();
        assert_eq!(decoded.channels().get(), 2);
        assert_eq!(decoded.sample_rate().get(), 24_000);
        assert_eq!(decoded.total_duration(), Some(Duration::from_secs(1)));
    }
}
