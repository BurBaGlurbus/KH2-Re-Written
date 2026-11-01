//! Playback off Windows through cpal: ALSA on Linux, which PipeWire and
//! PulseAudio answer through their ALSA plugins. A cpal stream may not leave
//! the thread that made it, so a thread of its own keeps it until dropped.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, Stream, StreamConfig};
use std::{
    io,
    sync::{
        Arc, Mutex,
        mpsc::{self, Sender},
    },
    thread::{self, JoinHandle},
};

/// Tried in this order; a device that refuses a fixed size picks its own.
const BUFFER_SIZES: [BufferSize; 2] = [BufferSize::Fixed(512), BufferSize::Default];

type Fill = Arc<Mutex<dyn FnMut(&mut [i16]) + Send>>;

pub struct Device {
    /// Dropping it ends the thread, and the stream with it.
    stop: Option<Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl Device {
    pub fn open(rate: u32, fill: impl FnMut(&mut [i16]) + Send + 'static) -> io::Result<Self> {
        let fill: Fill = Arc::new(Mutex::new(fill));
        let (opened_tx, opened) = mpsc::channel();
        let (stop, stopped) = mpsc::channel::<()>();
        let thread = thread::Builder::new().name("audio".into()).spawn(move || {
            match start(rate, &fill) {
                Ok(_stream) => {
                    let _ = opened_tx.send(Ok(()));
                    let _ = stopped.recv();
                }
                Err(e) => {
                    let _ = opened_tx.send(Err(e));
                }
            }
        })?;
        opened
            .recv()
            .map_err(|_| io::Error::other("the sound thread stopped"))??;
        Ok(Self {
            stop: Some(stop),
            thread: Some(thread),
        })
    }
}

/// The system's default output playing `fill`, 16-bit stereo at `rate`; floats
/// where the device has no 16-bit format.
fn start(rate: u32, fill: &Fill) -> io::Result<Stream> {
    let device = cpal::default_host()
        .default_output_device()
        .ok_or_else(|| io::Error::other("no default sound output device"))?;
    let mut last_error = String::new();
    for buffer_size in BUFFER_SIZES {
        let config = StreamConfig {
            channels: 2,
            sample_rate: rate,
            buffer_size,
        };
        let on_error = |e: cpal::Error| eprintln!("Sound output: {e}");
        let feed = fill.clone();
        let shorts = device.build_output_stream(
            config,
            move |out: &mut [i16], _: &cpal::OutputCallbackInfo| feed_shorts(&feed, out),
            on_error,
            None,
        );
        let feed = fill.clone();
        let mut mixed = Vec::new();
        let stream = shorts.or_else(|_| {
            device.build_output_stream(
                config,
                move |out: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    mixed.resize(out.len(), 0i16);
                    feed_shorts(&feed, &mut mixed);
                    for (o, s) in out.iter_mut().zip(&mixed) {
                        *o = f32::from(*s) / 32768.;
                    }
                },
                on_error,
                None,
            )
        });
        match stream {
            Ok(stream) => {
                stream.play().map_err(io::Error::other)?;
                return Ok(stream);
            }
            Err(e) => last_error = e.to_string(),
        }
    }
    Err(io::Error::other(last_error))
}

fn feed_shorts(fill: &Fill, out: &mut [i16]) {
    match fill.lock() {
        Ok(mut fill) => fill(out),
        Err(_) => out.fill(0),
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        drop(self.stop.take());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
