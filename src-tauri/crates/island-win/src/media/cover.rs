//! Normalize bounded session streams through WIC; never expose arbitrary SVG/HTML.
use super::*;
use sha2::{Digest, Sha256};
use std::sync::{atomic::Ordering, mpsc, Mutex};
use windows::{
    core::AgileReference,
    Storage::Streams::{Buffer, DataReader, IRandomAccessStreamReference, InputStreamOptions},
    Win32::{
        Graphics::Imaging::*,
        System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER},
        UI::Shell::SHCreateMemStream,
    },
};
pub(super) struct Cover {
    pub key: String,
    pub bytes: Arc<Vec<u8>>,
    pub tint: Option<[u8; 3]>,
}

enum DecoderState {
    Idle,
    Active,
    CoolingUntil(Instant),
}

// Process-wide so a stuck WIC call cannot accumulate workers on disable/re-enable.
static DECODER: Mutex<DecoderState> = Mutex::new(DecoderState::Idle);

struct DecodePermit;

impl DecodePermit {
    fn try_acquire() -> Option<Self> {
        let mut state = DECODER.lock().ok()?;
        match *state {
            DecoderState::Active => return None,
            DecoderState::CoolingUntil(deadline) if Instant::now() < deadline => return None,
            _ => {}
        }
        *state = DecoderState::Active;
        Some(Self)
    }
}

impl Drop for DecodePermit {
    fn drop(&mut self) {
        if let Ok(mut state) = DECODER.lock() {
            *state = DecoderState::CoolingUntil(Instant::now() + BACKGROUND_REFRESH_INTERVAL);
        }
    }
}

fn spawn_decode(
    permit: DecodePermit,
    decode: impl FnOnce() -> Option<Cover> + Send + 'static,
    changed: MediaChanged,
) -> AppResult<mpsc::Receiver<Option<Cover>>> {
    let (tx, rx) = mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("island-cover-decode".into())
        .spawn(move || {
            let cover = decode();
            let _ = tx.send(cover);
            drop(permit);
            changed();
        })?;
    Ok(rx)
}

pub(super) fn read_in_background(
    reference: &IRandomAccessStreamReference,
    cancel: Arc<AtomicBool>,
    changed: MediaChanged,
) -> AppResult<Option<mpsc::Receiver<Option<Cover>>>> {
    let Some(permit) = DecodePermit::try_acquire() else {
        return Ok(None);
    };
    let reference = AgileReference::new(reference).map_err(media_error)?;
    spawn_decode(
        permit,
        move || {
            let _apartment = super::native::Apartment::new().ok()?;
            let reference = reference.resolve().ok()?;
            read(&reference, &cancel, Instant::now() + Duration::from_secs(2)).ok()
        },
        changed,
    )
    .map(Some)
}

fn read(
    reference: &IRandomAccessStreamReference,
    cancel: &AtomicBool,
    deadline: Instant,
) -> AppResult<Cover> {
    if cancel.load(Ordering::Acquire) {
        return Err(AppError::new("timeout", "Das Lesen des Mediencovers wurde abgebrochen."));
    }
    let operation = reference.OpenReadAsync().map_err(media_error)?;
    let stream = wait(&operation, cancel, deadline)?;
    let result = (|| {
        let size = stream.Size().map_err(media_error)?;
        if size == 0 || size > 8 * 1024 * 1024 {
            return Err(AppError::new(
                "media_error",
                "Das Mediencover ist zu groß oder leer.",
            ));
        }
        let buffer = Buffer::Create(size as u32).map_err(media_error)?;
        let input = stream.GetInputStreamAt(0).map_err(media_error)?;
        let operation = input
            .ReadAsync(&buffer, size as u32, InputStreamOptions::None)
            .map_err(media_error)?;
        wait_status(
            || operation.Status(),
            || operation.Cancel(),
            cancel,
            deadline,
        )?;
        let bytes = operation.GetResults().map_err(media_error)?;
        if u64::from(bytes.Length().map_err(media_error)?) != size {
            return Err(AppError::new(
                "media_error",
                "Der Coverstream ist unvollständig oder hat sich geändert.",
            ));
        }
        let reader = DataReader::FromBuffer(&bytes).map_err(media_error)?;
        let mut bytes = vec![0; bytes.Length().map_err(media_error)? as usize];
        reader.ReadBytes(&mut bytes).map_err(media_error)?;
        let _ = reader.Close();
        if cancel.load(Ordering::Acquire) || Instant::now() >= deadline {
            return Err(AppError::new(
                "timeout",
                "Das Mediencover wurde nicht rechtzeitig gelesen.",
            ));
        }
        let (png, tint) = normalize_image(&bytes, 256, true)?;
        Ok(Cover {
            key: format!("media-cover-{:x}", Sha256::digest(&png)),
            bytes: Arc::new(png),
            tint,
        })
    })();
    let _ = stream.Close();
    result
}
#[cfg(test)]
pub(crate) fn normalize(bytes: &[u8], max_edge: u32) -> AppResult<Vec<u8>> {
    normalize_image(bytes, max_edge, false).map(|(png, _)| png)
}

type NormalizedImage = (Vec<u8>, Option<[u8; 3]>);

fn normalize_image(bytes: &[u8], max_edge: u32, tint: bool) -> AppResult<NormalizedImage> {
    let result = (|| -> windows::core::Result<NormalizedImage> {
        unsafe {
            let stream =
                SHCreateMemStream(Some(bytes)).ok_or_else(windows::core::Error::from_thread)?;
            let factory: IWICImagingFactory =
                CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
            let decoder = factory.CreateDecoderFromStream(
                &stream,
                std::ptr::null(),
                WICDecodeMetadataCacheOnDemand,
            )?;
            let frame = decoder.GetFrame(0)?;
            let (mut width, mut height) = (0, 0);
            frame.GetSize(&mut width, &mut height)?;
            if width == 0 || height == 0 || u64::from(width) * u64::from(height) > 32 * 1024 * 1024
            {
                return Err(windows::core::Error::from_hresult(windows::core::HRESULT(
                    0x80070057u32 as i32,
                )));
            }
            let scale = (f64::from(max_edge) / f64::from(width.max(height))).min(1.);
            let (width, height) = (
                (f64::from(width) * scale).round().max(1.) as u32,
                (f64::from(height) * scale).round().max(1.) as u32,
            );
            let scaler = factory.CreateBitmapScaler()?;
            scaler.Initialize(&frame, width, height, WICBitmapInterpolationModeFant)?;
            let converter = factory.CreateFormatConverter()?;
            converter.Initialize(
                &scaler,
                &GUID_WICPixelFormat32bppRGBA,
                WICBitmapDitherTypeNone,
                None,
                0.,
                WICBitmapPaletteTypeCustom,
            )?;
            let mut pixels = vec![0; (width * height * 4) as usize];
            converter.CopyPixels(std::ptr::null(), width * 4, &mut pixels)?;
            let mut output = Vec::new();
            {
                let mut encoder = png::Encoder::new(&mut output, width, height);
                encoder.set_color(png::ColorType::Rgba);
                encoder.set_depth(png::BitDepth::Eight);
                let mut writer = encoder.write_header().map_err(|_| {
                    windows::core::Error::from_hresult(windows::core::HRESULT(0x80004005u32 as i32))
                })?;
                writer.write_image_data(&pixels).map_err(|_| {
                    windows::core::Error::from_hresult(windows::core::HRESULT(0x80004005u32 as i32))
                })?;
            }
            Ok((output, tint.then(|| cover_tint(&pixels)).flatten()))
        }
    })();
    result.map_err(media_error)
}

fn cover_tint(pixels: &[u8]) -> Option<[u8; 3]> {
    let mut palette = [[0u32; 4]; 512];
    for pixel in pixels.chunks_exact(4).step_by(7) {
        let [r, g, b, a] = [pixel[0], pixel[1], pixel[2], pixel[3]];
        let high = r.max(g).max(b);
        let low = r.min(g).min(b);
        if a < 128 || high < 32 || low > 239 {
            continue;
        }
        let index = (usize::from(r >> 5) << 6) | (usize::from(g >> 5) << 3) | usize::from(b >> 5);
        let entry = &mut palette[index];
        entry[0] += 12 + u32::from(high - low);
        for (sum, component) in entry[1..].iter_mut().zip([r, g, b]) {
            *sum += u32::from(component);
        }
    }
    let [_, red, green, blue] = palette.into_iter().max_by_key(|entry| entry[0])?;
    let high = red.max(green).max(blue);
    if high == 0 {
        return None;
    }
    Some([red, green, blue].map(|component| (component * 210 / high) as u8))
}

#[test]
fn cover_job_has_one_slot_and_decodes_a_winrt_thumbnail_off_thread() {
    use windows::{
        core::Interface,
        Storage::Streams::{DataWriter, InMemoryRandomAccessStream, RandomAccessStreamReference},
    };

    let caller = std::thread::current().id();
    let (release, gate) = mpsc::sync_channel(1);
    let (finished, done) = mpsc::sync_channel(1);
    let result = spawn_decode(
        DecodePermit::try_acquire().unwrap(),
        move || {
            assert_ne!(std::thread::current().id(), caller);
            gate.recv_timeout(Duration::from_secs(3)).unwrap();
            None
        },
        Arc::new(move || finished.send(()).unwrap()),
    )
    .unwrap();
    assert!(DecodePermit::try_acquire().is_none());
    assert!(matches!(result.try_recv(), Err(mpsc::TryRecvError::Empty)));
    release.send(()).unwrap();
    done.recv_timeout(Duration::from_secs(3)).unwrap();
    assert!(result.recv().unwrap().is_none());

    let _apartment = super::native::Apartment::new().unwrap();
    let stream = InMemoryRandomAccessStream::new().unwrap();
    let writer = DataWriter::CreateDataWriter(&stream.GetOutputStreamAt(0).unwrap()).unwrap();
    writer
        .WriteBytes(include_bytes!("../../../../icons/icon.png"))
        .unwrap();
    let cancel = Arc::new(AtomicBool::new(false));
    wait(
        &writer.StoreAsync().unwrap(),
        &cancel,
        Instant::now() + Duration::from_secs(2),
    )
    .unwrap();
    writer.DetachStream().unwrap();
    stream.Seek(0).unwrap();
    let reference = RandomAccessStreamReference::CreateFromStream(&stream)
        .unwrap()
        .cast()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    let result = loop {
        if let Some(result) = read_in_background(&reference, cancel.clone(), Arc::new(|| {})).unwrap() {
            break result;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    };
    let cover = result.recv_timeout(Duration::from_secs(3)).unwrap().unwrap();
    assert!(cover.bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    assert!(cover.tint.is_some());
}

#[test]
fn wic_cover_normalizes_to_bounded_png_and_rejects_non_images() {
    use windows::Win32::System::WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED};
    unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.unwrap();
    struct Apartment;
    impl Drop for Apartment {
        fn drop(&mut self) {
            unsafe {
                RoUninitialize();
            }
        }
    }
    let _apartment = Apartment;
    let source = include_bytes!("../../../../icons/icon.png");
    let (output, tint) = normalize_image(source, 256, true).unwrap();
    let [red, green, blue] = tint.unwrap();
    assert!(blue > red && blue >= green);
    let decoder = png::Decoder::new(std::io::Cursor::new(output));
    let reader = decoder.read_info().unwrap();
    assert!(reader.info().width > 0 && reader.info().width <= 256);
    assert!(reader.info().height > 0 && reader.info().height <= 256);
    assert_eq!(reader.info().color_type, png::ColorType::Rgba);
    assert!(normalize(
        b"<svg xmlns='http://www.w3.org/2000/svg'><script>test</script></svg>",
        256
    )
    .is_err());
    assert!(normalize(&[], 256).is_err());
}
