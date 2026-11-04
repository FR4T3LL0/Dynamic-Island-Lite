//! Core Audio stays on its owning MTA thread. Callbacks only schedule a refresh.
use island_core::{
    audio::{validate_volume, AudioDevice, AudioInfo},
    error::{AppError, AppResult},
};
use std::{marker::PhantomData, rc::Rc, sync::Arc};
use windows::{
    core::{implement, PCWSTR, PWSTR},
    Win32::{
        Devices::FunctionDiscovery::PKEY_Device_FriendlyName,
        Foundation::PROPERTYKEY,
        Media::Audio::{
            eMultimedia, eRender, EDataFlow, ERole,
            Endpoints::{
                IAudioEndpointVolume, IAudioEndpointVolumeCallback,
                IAudioEndpointVolumeCallback_Impl,
            },
            IMMDevice, IMMDeviceEnumerator, IMMNotificationClient, IMMNotificationClient_Impl,
            MMDeviceEnumerator, AUDIO_VOLUME_NOTIFICATION_DATA, DEVICE_STATE, DEVICE_STATE_ACTIVE,
        },
        System::Com::{
            CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize,
            StructuredStorage::{PropVariantClear, PropVariantToStringAlloc},
            CLSCTX_ALL, COINIT_MULTITHREADED, STGM_READ,
        },
    },
};

pub type AudioChanged = Arc<dyn Fn() + Send + Sync>;

fn audio_error(context: &str, error: windows::core::Error) -> AppError {
    AppError::new(
        "audio_error",
        format!("{context} (Windows {:08X}).", error.code().0 as u32),
    )
}

struct Apartment(PhantomData<Rc<()>>);
impl Apartment {
    fn new() -> AppResult<Self> {
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok() }
            .map_err(|e| audio_error("Die Audioverbindung konnte nicht initialisiert werden", e))?;
        Ok(Self(PhantomData))
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() }
    }
}

/// Take ownership of an output allocated with CoTaskMemAlloc, including errors.
fn take_string(value: PWSTR) -> AppResult<String> {
    if value.is_null() {
        return Err(AppError::new(
            "audio_error",
            "Windows lieferte keine Gerätekennung.",
        ));
    }
    let result = unsafe { value.to_string() }
        .map_err(|_| AppError::new("audio_error", "Die Gerätekennung ist ungültig."));
    unsafe { CoTaskMemFree(Some(value.0.cast())) };
    result
}
fn device_id(device: &IMMDevice) -> AppResult<String> {
    take_string(
        unsafe { device.GetId() }
            .map_err(|e| audio_error("Die Gerätekennung konnte nicht gelesen werden", e))?,
    )
}
fn device_name(device: &IMMDevice) -> AppResult<String> {
    unsafe {
        let store = device
            .OpenPropertyStore(STGM_READ)
            .map_err(|e| audio_error("Der Gerätename konnte nicht gelesen werden", e))?;
        let mut value = store
            .GetValue(&PKEY_Device_FriendlyName)
            .map_err(|e| audio_error("Der Gerätename fehlt", e))?;
        let allocated = PropVariantToStringAlloc(&value);
        let _ = PropVariantClear(&mut value);
        take_string(allocated.map_err(|e| audio_error("Der Gerätename ist ungültig", e))?)
    }
}
fn wide(value: &str) -> AppResult<Vec<u16>> {
    if value.is_empty() || value.len() > 4096 || value.contains('\0') {
        return Err(AppError::new(
            "invalid_argument",
            "Das Ausgabegerät ist ungültig.",
        ));
    }
    Ok(value.encode_utf16().chain(Some(0)).collect())
}

// This private ABI is deliberately isolated. Only SetDefaultEndpoint is called.
#[implement(IMMNotificationClient)]
struct DeviceNotifications(AudioChanged);
impl IMMNotificationClient_Impl for DeviceNotifications_Impl {
    fn OnDeviceStateChanged(&self, _: &PCWSTR, _: DEVICE_STATE) -> windows::core::Result<()> {
        (self.0)();
        Ok(())
    }
    fn OnDeviceAdded(&self, _: &PCWSTR) -> windows::core::Result<()> {
        (self.0)();
        Ok(())
    }
    fn OnDeviceRemoved(&self, _: &PCWSTR) -> windows::core::Result<()> {
        (self.0)();
        Ok(())
    }
    fn OnDefaultDeviceChanged(
        &self,
        flow: EDataFlow,
        _: ERole,
        _: &PCWSTR,
    ) -> windows::core::Result<()> {
        if flow == eRender {
            (self.0)();
        }
        Ok(())
    }
    fn OnPropertyValueChanged(&self, _: &PCWSTR, _: &PROPERTYKEY) -> windows::core::Result<()> {
        (self.0)();
        Ok(())
    }
}
#[implement(IAudioEndpointVolumeCallback)]
struct VolumeNotifications(AudioChanged);
impl IAudioEndpointVolumeCallback_Impl for VolumeNotifications_Impl {
    fn OnNotify(&self, _: *mut AUDIO_VOLUME_NOTIFICATION_DATA) -> windows::core::Result<()> {
        (self.0)();
        Ok(())
    }
}

struct VolumeSubscription {
    volume: IAudioEndpointVolume,
    callback: Option<IAudioEndpointVolumeCallback>,
    id: String,
}
impl Drop for VolumeSubscription {
    fn drop(&mut self) {
        let _ = self.close();
    }
}
impl VolumeSubscription {
    fn close(&mut self) -> AppResult<()> {
        if let Some(callback) = &self.callback {
            unsafe { self.volume.UnregisterControlChangeNotify(callback) }.map_err(|e| {
                audio_error("Lautstärkeänderungen konnten nicht abgemeldet werden", e)
            })?;
            self.callback = None;
        }
        Ok(())
    }
}

pub struct AudioAdapter {
    // Apartment must be dropped last, after all COM objects and subscriptions.
    volume: Option<VolumeSubscription>,
    notifications: Option<IMMNotificationClient>,
    enumerator: IMMDeviceEnumerator,
    changed: Option<AudioChanged>,
    _apartment: Apartment,
}
impl AudioAdapter {
    pub fn subscription_count(&self) -> usize {
        usize::from(self.notifications.is_some())
            + usize::from(
                self.volume
                    .as_ref()
                    .is_some_and(|volume| volume.callback.is_some()),
            )
    }
    fn close_volume(&mut self) -> AppResult<()> {
        if let Some(volume) = &mut self.volume {
            volume.close()?;
        }
        self.volume = None;
        Ok(())
    }
    /// Explicit stop reports unregister failures before the COM owner exits.
    /// Repeated calls are safe; Drop retries any registration still retained.
    pub fn close(&mut self) -> AppResult<()> {
        let volume = self.close_volume();
        let devices = if let Some(callback) = &self.notifications {
            unsafe {
                self.enumerator
                    .UnregisterEndpointNotificationCallback(callback)
            }
            .map_err(|e| audio_error("Geräteänderungen konnten nicht abgemeldet werden", e))
        } else {
            Ok(())
        };
        if devices.is_ok() {
            self.notifications = None;
        }
        match (volume, devices) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(a), Err(b)) => Err(AppError::new(
                "audio_error",
                format!("{} {}", a.message, b.message),
            )),
            (Err(error), _) | (_, Err(error)) => Err(error),
        }
    }
    pub fn new(changed: Option<AudioChanged>) -> AppResult<Self> {
        let apartment = Apartment::new()?;
        let enumerator: IMMDeviceEnumerator =
            unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
                .map_err(|e| audio_error("Die Audioausgaben konnten nicht geöffnet werden", e))?;
        let notifications = changed
            .as_ref()
            .map(|callback| IMMNotificationClient::from(DeviceNotifications(callback.clone())));
        if let Some(callback) = &notifications {
            unsafe { enumerator.RegisterEndpointNotificationCallback(callback) }
                .map_err(|e| audio_error("Geräteänderungen konnten nicht abonniert werden", e))?;
        }
        Ok(Self {
            enumerator,
            notifications,
            changed,
            volume: None,
            _apartment: apartment,
        })
    }
    fn endpoint(&self, id: &str) -> AppResult<IMMDevice> {
        let name = wide(id)?;
        let device = unsafe { self.enumerator.GetDevice(PCWSTR(name.as_ptr())) }
            .map_err(|e| audio_error("Das Ausgabegerät ist nicht mehr verfügbar", e))?;
        if unsafe { device.GetState() }
            .map_err(|e| audio_error("Der Gerätezustand konnte nicht gelesen werden", e))?
            != DEVICE_STATE_ACTIVE
        {
            return Err(AppError::new(
                "not_found",
                "Das Ausgabegerät ist nicht aktiv.",
            ));
        }
        // Accept playback devices from the active enumeration, never arbitrary IDs.
        let active = self.devices()?;
        if !active.iter().any(|d| d.id == id) {
            return Err(AppError::new(
                "not_found",
                "Die Audioausgabe ist nicht verfügbar.",
            ));
        }
        Ok(device)
    }
    pub fn devices(&self) -> AppResult<Vec<AudioDevice>> {
        let devices = unsafe {
            self.enumerator
                .EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)
        }
        .map_err(|e| audio_error("Die Audioausgaben konnten nicht aufgelistet werden", e))?;
        let count = unsafe { devices.GetCount() }.map_err(|e| {
            audio_error(
                "Die Anzahl der Audioausgaben konnte nicht gelesen werden",
                e,
            )
        })?;
        let mut result = Vec::new();
        for index in 0..count {
            let device = unsafe { devices.Item(index) }
                .map_err(|e| audio_error("Eine Audioausgabe ist nicht verfügbar", e))?;
            result.push(AudioDevice {
                id: device_id(&device)?,
                name: device_name(&device).unwrap_or_else(|_| "Audioausgabe".into()),
            });
        }
        result.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
        Ok(result)
    }
    fn volume_for(&self, id: &str) -> AppResult<IAudioEndpointVolume> {
        unsafe { self.endpoint(id)?.Activate(CLSCTX_ALL, None) }
            .map_err(|e| audio_error("Die Systemlautstärke ist nicht verfügbar", e))
    }
    pub fn snapshot(&mut self) -> AppResult<AudioInfo> {
        let devices = self.devices()?;
        let id = self.default_endpoint()?.unwrap_or_default();
        if id.is_empty() {
            self.close_volume()?;
            return Ok(AudioInfo {
                volume: 0.0,
                muted: false,
                default_device: id,
                devices,
            });
        }
        if self.volume.as_ref().is_none_or(|v| v.id != id) {
            self.close_volume()?;
            let volume = self.volume_for(&id)?;
            let callback: IAudioEndpointVolumeCallback =
                VolumeNotifications(self.changed.clone().unwrap_or_else(|| Arc::new(|| {}))).into();
            unsafe { volume.RegisterControlChangeNotify(&callback) }.map_err(|e| {
                audio_error("Lautstärkeänderungen konnten nicht abonniert werden", e)
            })?;
            self.volume = Some(VolumeSubscription {
                volume,
                callback: Some(callback),
                id: id.clone(),
            });
        }
        let volume = &self
            .volume
            .as_ref()
            .expect("volume initialized above")
            .volume;
        Ok(AudioInfo {
            volume: unsafe { volume.GetMasterVolumeLevelScalar() }
                .map_err(|e| audio_error("Die Lautstärke konnte nicht gelesen werden", e))?
                * 100.0,
            muted: unsafe { volume.GetMute() }
                .map_err(|e| audio_error("Die Stummschaltung konnte nicht gelesen werden", e))?
                .as_bool(),
            default_device: id,
            devices,
        })
    }
    pub fn set_volume(&mut self, id: &str, volume: f32) -> AppResult<AudioInfo> {
        let scalar = validate_volume(volume)?;
        self.require_default(id)?;
        unsafe {
            self.volume_for(id)?
                .SetMasterVolumeLevelScalar(scalar, std::ptr::null())
        }
        .map_err(|e| audio_error("Die Systemlautstärke konnte nicht gesetzt werden", e))?;
        self.snapshot()
    }
    fn require_default(&self, id: &str) -> AppResult<()> {
        if self.default_endpoint()?.as_deref() != Some(id) {
            return Err(AppError::new(
                "conflict",
                "Die Standardausgabe hat sich geändert. Bitte erneut versuchen.",
            ));
        }
        Ok(())
    }
    fn default_endpoint(&self) -> AppResult<Option<String>> {
        match unsafe {
            self.enumerator
                .GetDefaultAudioEndpoint(eRender, eMultimedia)
        } {
            Ok(device) => device_id(&device).map(Some),
            Err(error) if error.code().0 as u32 == 0x80070490 => Ok(None),
            Err(error) => Err(audio_error(
                "Die Standardausgabe konnte nicht gelesen werden",
                error,
            )),
        }
    }
}
impl Drop for AudioAdapter {
    fn drop(&mut self) {
        let _ = self.close();
    }
}
