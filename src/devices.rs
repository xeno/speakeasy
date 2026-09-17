use cpal::traits::{DeviceTrait, HostTrait};
use nokhwa::query;
use nokhwa::utils::{ApiBackend, CameraIndex};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VideoSource {
    pub index: CameraIndex,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioSource {
    pub name: String,
}

pub fn list_cameras() -> Result<Vec<VideoSource>, String> {
    let cameras = query(ApiBackend::Auto).map_err(|err| err.to_string())?;
    Ok(cameras
        .into_iter()
        .map(|info| VideoSource {
            index: info.index().clone(),
            name: info.human_name(),
        })
        .collect())
}

pub fn list_mics() -> Result<Vec<AudioSource>, String> {
    let host = cpal::default_host();
    let mut mics = Vec::new();

    if let Some(default) = host.default_input_device() {
        mics.push(AudioSource {
            name: device_name(&default),
        });
    }

    let devices = host.input_devices().map_err(|err| err.to_string())?;
    for device in devices {
        let name = device_name(&device);
        if !mics.iter().any(|mic| mic.name == name) {
            mics.push(AudioSource { name });
        }
    }

    Ok(mics)
}

pub fn find_input_device(name: &str) -> Result<cpal::Device, String> {
    let host = cpal::default_host();
    if let Some(default) = host.default_input_device()
        && device_name(&default) == name
    {
        return Ok(default);
    }

    host.input_devices()
        .map_err(|err| err.to_string())?
        .find(|device| device_name(device) == name)
        .ok_or_else(|| format!("Microphone not found: {name}"))
}

fn device_name(device: &cpal::Device) -> String {
    device
        .description()
        .map(|info| info.name().to_string())
        .unwrap_or_else(|_| device.to_string())
}
