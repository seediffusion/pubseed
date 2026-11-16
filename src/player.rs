//! Playback of uploaded audios and live streams.
//!
//! Opening a URL and probing its format is slow, so `open` runs on a worker
//! thread and hands the decoder back to the UI thread, which owns `Player`.

use crate::api::{Api, Result};
use crate::stretch::Stretch;
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Source};
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::os::windows::fs::FileExt;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

pub trait Media: Read + Seek + Send + Sync {}
impl<T: Read + Seek + Send + Sync> Media for T {}

pub type Decoded = Decoder<Box<dyn Media>>;

/// An audio file downloading into a temporary file, readable and seekable
/// while it arrives. Reads past what has arrived wait for it.
struct Download {
    file: File,
    /// Bytes written so far, and whether the download has ended.
    progress: Mutex<(u64, bool)>,
    arrived: Condvar,
    cancel: AtomicBool,
}

struct FileMedia {
    download: Arc<Download>,
    position: u64,
}

impl Read for FileMedia {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut progress = self.download.progress.lock().unwrap();
        while progress.0 <= self.position && !progress.1 {
            progress = self.download.arrived.wait(progress).unwrap();
        }
        let available = progress.0.saturating_sub(self.position);
        drop(progress);
        let wanted = buf.len().min(available as usize);
        let read = self
            .download
            .file
            .seek_read(&mut buf[..wanted], self.position)?;
        self.position += read as u64;
        Ok(read)
    }
}

impl Seek for FileMedia {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let target = match to {
            SeekFrom::Start(offset) => offset as i64,
            SeekFrom::Current(offset) => self.position as i64 + offset,
            SeekFrom::End(offset) => {
                let mut progress = self.download.progress.lock().unwrap();
                while !progress.1 {
                    progress = self.download.arrived.wait(progress).unwrap();
                }
                progress.0 as i64 + offset
            }
        };
        self.position = target.max(0) as u64;
        Ok(self.position)
    }
}

impl Drop for FileMedia {
    fn drop(&mut self) {
        self.download.cancel.store(true, Ordering::Relaxed);
    }
}

/// A live stream: read straight off the connection, never seekable.
struct LiveMedia(Mutex<reqwest::blocking::Response>);

impl Read for LiveMedia {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.get_mut().unwrap().read(buf)
    }
}

impl Seek for LiveMedia {
    fn seek(&mut self, _: SeekFrom) -> io::Result<u64> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "a live stream cannot seek",
        ))
    }
}

fn start_download(mut response: reqwest::blocking::Response) -> io::Result<FileMedia> {
    let download = Arc::new(Download {
        file: tempfile::tempfile()?,
        progress: Mutex::new((0, false)),
        arrived: Condvar::new(),
        cancel: AtomicBool::new(false),
    });
    let writer = download.clone();
    std::thread::spawn(move || {
        let mut chunk = vec![0u8; 64 * 1024];
        let mut written = 0u64;
        while !writer.cancel.load(Ordering::Relaxed) {
            let read = match response.read(&mut chunk) {
                Ok(read) if read > 0 => read,
                _ => break,
            };
            if writer.file.seek_write(&chunk[..read], written).is_err() {
                break;
            }
            written += read as u64;
            writer.progress.lock().unwrap().0 = written;
            writer.arrived.notify_all();
        }
        writer.progress.lock().unwrap().1 = true;
        writer.arrived.notify_all();
    });
    Ok(FileMedia {
        download,
        position: 0,
    })
}

/// Opens the first of `urls` that can be decoded. Blocks on the network.
pub fn open(api: &Api, urls: &[String], live: bool) -> Result<Decoded> {
    let mut last_error = "Nothing to play".to_string();
    for url in urls {
        let response = match api.open_audio(url) {
            Ok(response) => response,
            Err(e) => {
                last_error = e;
                continue;
            }
        };
        let mime = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_string();
        let mut builder = Decoder::builder().with_seekable(!live);
        if mime.starts_with("audio/") {
            builder = builder.with_mime_type(&mime);
        }
        if let Some(length) = response.content_length().filter(|_| !live) {
            builder = builder.with_byte_len(length);
        }
        let media: Box<dyn Media> = if live {
            Box::new(LiveMedia(Mutex::new(response)))
        } else {
            match start_download(response) {
                Ok(media) => Box::new(media),
                Err(e) => return Err(format!("Could not create a temporary file: {e}")),
            }
        };
        match builder.with_data(media).build() {
            Ok(decoded) => return Ok(decoded),
            Err(e) => last_error = format!("This audio could not be played: {e}"),
        }
    }
    Err(last_error)
}

pub struct Player {
    _device: Option<MixerDeviceSink>,
    sink: Option<rodio::Player>,
    /// Bumped by every `begin`, so a decoder that arrives late is dropped.
    generation: u64,
    started: bool,
    pub duration: Option<Duration>,
    /// The speed, shared with the stretcher of whatever is playing.
    speed: Arc<AtomicU32>,
    /// Where the stretcher has reached in the audio, in milliseconds.
    position_ms: Arc<AtomicU64>,
    /// Whether what is playing goes through the stretcher. Live streams do not.
    stretched: bool,
}

impl Player {
    pub fn new(volume: u32, speed: f32) -> Self {
        let device = DeviceSinkBuilder::open_default_sink().ok();
        let sink = device
            .as_ref()
            .map(|device| rodio::Player::connect_new(device.mixer()));
        let player = Self {
            _device: device,
            sink,
            generation: 0,
            started: false,
            duration: None,
            speed: Arc::new(AtomicU32::new(speed.to_bits())),
            position_ms: Arc::new(AtomicU64::new(0)),
            stretched: false,
        };
        player.set_volume(volume);
        player
    }

    /// Stops whatever is playing and returns the ticket for the next `start`.
    pub fn begin(&mut self) -> u64 {
        self.stop();
        self.generation
    }

    /// Plays a decoder opened for `ticket`, unless something newer was begun.
    pub fn start(&mut self, ticket: u64, decoded: Decoded, live: bool) -> Result<bool> {
        if ticket != self.generation {
            return Ok(false);
        }
        let sink = self
            .sink
            .as_ref()
            .ok_or("No audio output device was found")?;
        self.duration = decoded.total_duration();
        self.stretched = !live;
        if live {
            sink.append(decoded);
        } else {
            sink.append(Stretch::new(
                decoded,
                self.speed.clone(),
                self.position_ms.clone(),
            ));
        }
        sink.play();
        self.started = true;
        Ok(true)
    }

    pub fn is_current(&self, ticket: u64) -> bool {
        ticket == self.generation
    }

    pub fn stop(&mut self) {
        self.generation += 1;
        self.started = false;
        self.duration = None;
        if let Some(sink) = &self.sink {
            sink.clear();
        }
    }

    pub fn is_loaded(&self) -> bool {
        self.started
    }

    /// True once the audio that was started has played to its end.
    pub fn finished(&self) -> bool {
        self.started && self.sink.as_ref().is_some_and(|sink| sink.empty())
    }

    pub fn is_paused(&self) -> bool {
        self.sink.as_ref().is_none_or(|sink| sink.is_paused())
    }

    pub fn set_paused(&self, paused: bool) {
        if let Some(sink) = &self.sink {
            if paused { sink.pause() } else { sink.play() }
        }
    }

    pub fn set_speed(&self, speed: f32) {
        self.speed.store(speed.to_bits(), Ordering::Relaxed);
    }

    pub fn position(&self) -> Duration {
        // Time spent playing is not the place reached once the speed is not 1.
        if self.stretched {
            return Duration::from_millis(self.position_ms.load(Ordering::Relaxed));
        }
        self.sink
            .as_ref()
            .map(|sink| sink.get_pos())
            .unwrap_or_default()
    }

    pub fn seek(&self, to: Duration) {
        let to = self.duration.map_or(to, |duration| to.min(duration));
        if let Some(sink) = &self.sink {
            let _ = sink.try_seek(to);
        }
    }

    pub fn set_volume(&self, percent: u32) {
        if let Some(sink) = &self.sink {
            sink.set_volume(percent.min(100) as f32 / 100.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::Source as Feed;

    /// Downloads and decodes the newest audio on the real site.
    /// Run with `cargo test live_site -- --ignored`.
    #[test]
    #[ignore]
    fn live_site_audio_decodes() {
        let api = Api::new("https://audiopub.site", "https://live.audiopub.site", None);
        let audio = api.listing(&Feed::Home, 0, 1).unwrap().audios.remove(0);
        let urls = [
            format!("{}/audio/{}", api.base, audio.id),
            format!("{}/audio/{}.aac", api.base, audio.id),
        ];
        let mut decoded = open(&api, &urls, false).unwrap();
        assert!(decoded.sample_rate().get() > 0);
        assert!(decoded.by_ref().take(48_000).count() > 0);
    }
}
