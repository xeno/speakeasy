use crate::devices::find_input_device;
use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{SampleFormat, SizedSample};
use hound::{WavSpec, WavWriter};
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use std::sync::{Arc, Mutex};

type WavOut = WavWriter<BufWriter<File>>;

pub struct MicCapture {
    stream: cpal::Stream,
    writer: Arc<Mutex<Option<WavOut>>>,
}

impl MicCapture {
    pub fn start(device_name: &str, path: &Path) -> Result<Self, String> {
        let device = find_input_device(device_name)?;
        let supported = device
            .default_input_config()
            .map_err(|err| format!("Microphone config: {err}"))?;
        let spec = WavSpec {
            channels: supported.channels(),
            sample_rate: supported.sample_rate(),
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let file = BufWriter::new(File::create(path).map_err(|err| err.to_string())?);
        let writer = WavWriter::new(file, spec).map_err(|err| err.to_string())?;
        let writer = Arc::new(Mutex::new(Some(writer)));
        let config = supported.config();
        let stream = match supported.sample_format() {
            SampleFormat::I16 => build_stream::<i16>(&device, &config, &writer)?,
            SampleFormat::I8 => build_stream::<i8>(&device, &config, &writer)?,
            SampleFormat::I32 => build_stream::<i32>(&device, &config, &writer)?,
            SampleFormat::U8 => build_stream::<u8>(&device, &config, &writer)?,
            SampleFormat::U16 => build_stream::<u16>(&device, &config, &writer)?,
            SampleFormat::F32 => build_stream::<f32>(&device, &config, &writer)?,
            SampleFormat::F64 => build_stream::<f64>(&device, &config, &writer)?,
            other => {
                return Err(format!(
                    "This microphone uses an unsupported sample format: {other:?}"
                ));
            }
        };
        stream.play().map_err(|err| format!("Microphone: {err}"))?;
        Ok(Self { stream, writer })
    }

    pub fn finish(self) -> Result<(), String> {
        drop(self.stream);
        let writer = self
            .writer
            .lock()
            .map_err(|_| "Microphone writer lock".to_string())?
            .take();
        if let Some(writer) = writer {
            writer.finalize().map_err(|err| err.to_string())?;
        }
        Ok(())
    }
}

fn build_stream<T: SizedSample + ToI16>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    writer: &Arc<Mutex<Option<WavOut>>>,
) -> Result<cpal::Stream, String> {
    let writer = Arc::clone(writer);
    device
        .build_input_stream(
            config.clone(),
            move |samples: &[T], _| {
                if let Ok(mut guard) = writer.lock()
                    && let Some(wav) = guard.as_mut()
                {
                    for sample in samples {
                        let _ = wav.write_sample(sample.to_i16());
                    }
                }
            },
            |err| eprintln!("SpeakEasy microphone error: {err}"),
            None,
        )
        .map_err(|err| format!("Microphone stream: {err}"))
}

trait ToI16: Copy {
    fn to_i16(self) -> i16;
}

impl ToI16 for i16 {
    fn to_i16(self) -> i16 {
        self
    }
}

impl ToI16 for i8 {
    fn to_i16(self) -> i16 {
        i16::from(self) << 8
    }
}

impl ToI16 for i32 {
    fn to_i16(self) -> i16 {
        (self >> 16) as i16
    }
}

impl ToI16 for u8 {
    fn to_i16(self) -> i16 {
        (i16::from(self) - 128) << 8
    }
}

impl ToI16 for u16 {
    fn to_i16(self) -> i16 {
        (i32::from(self) - 32_768) as i16
    }
}

impl ToI16 for f32 {
    fn to_i16(self) -> i16 {
        (self.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16
    }
}

impl ToI16 for f64 {
    fn to_i16(self) -> i16 {
        (self.clamp(-1.0, 1.0) * f64::from(i16::MAX)) as i16
    }
}
