use super::{RecordOptions, RecordResult, RecordState, RecordStatus, DISK_FAILED, ENCODER_FAILED};
use std::mem::{size_of, ManuallyDrop};
use std::path::Path;
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use windows::core::{factory, Interface, HSTRING, PCWSTR};
use windows::Foundation::TypedEventHandler;
use windows::Graphics::Capture::{
    Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession,
};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Win32::Foundation::{BOOL, HANDLE, HMODULE, POINT, RECT};
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::{Common::*, IDXGIDevice};
use windows::Win32::Graphics::Gdi::{MonitorFromPoint, MONITOR_DEFAULTTONULL};
use windows::Win32::Media::Audio::*;
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::*;
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
use windows::Win32::System::WinRT::Direct3D11::{
    CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use windows::Win32::System::WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED};

type WinResult<T> = windows::core::Result<T>;
const TICKS: i64 = 10_000_000;

pub fn supported() -> bool {
    // CreateForMonitor was introduced in Windows 10 1903 (build 18362).
    #[repr(C)]
    struct Version {
        size: u32,
        major: u32,
        minor: u32,
        build: u32,
        platform: u32,
        text: [u16; 128],
    }
    #[link(name = "ntdll")]
    extern "system" {
        fn RtlGetVersion(version: *mut Version) -> i32;
    }
    let mut v = Version {
        size: size_of::<Version>() as u32,
        major: 0,
        minor: 0,
        build: 0,
        platform: 0,
        text: [0; 128],
    };
    (unsafe { RtlGetVersion(&mut v) == 0 && v.build >= 18362 })
        && GraphicsCaptureSession::IsSupported().unwrap_or(false)
}

fn failure(error: windows::core::Error) -> String {
    log::error!("screen record native: {error:?}");
    match error.code().0 as u32 {
        0x80070005 => "screen_record.permission_denied",
        0x80070070 | 0x80070003 | 0x80070020 => DISK_FAILED,
        _ => ENCODER_FAILED,
    }
    .into()
}

enum Command {
    Pause(bool, mpsc::SyncSender<()>),
    Stop,
    Cancel,
}

pub struct Recorder {
    sender: mpsc::Sender<Command>,
    worker: Option<JoinHandle<Result<u64, String>>>,
    status: Arc<Mutex<RecordStatus>>,
    options: RecordOptions,
}

impl Recorder {
    pub fn start(options: RecordOptions) -> Result<Self, String> {
        if options.microphone {
            return Err("screen_record.microphone_unsupported".into());
        }
        let status = Arc::new(Mutex::new(RecordStatus {
            state: RecordState::Recording,
            elapsed_ms: 0,
            width: options.width,
            height: options.height,
            message: None,
        }));
        let (sender, receiver) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let shared = status.clone();
        let opts = options.clone();
        let worker = thread::spawn(move || {
            let result = unsafe { record(&opts, receiver, &shared, &ready_tx) }.map_err(failure);
            if let Err(err) = &result {
                let mut state = shared.lock().unwrap();
                state.state = RecordState::Error;
                state.message = Some(err.clone());
                let _ = ready_tx.send(Err(err.clone()));
            }
            result
        });
        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Self {
                sender,
                worker: Some(worker),
                status,
                options,
            }),
            other => {
                let _ = worker.join();
                Err(other
                    .ok()
                    .and_then(Result::err)
                    .unwrap_or_else(|| ENCODER_FAILED.into()))
            }
        }
    }

    pub fn status(&self) -> RecordStatus {
        self.status.lock().unwrap().clone()
    }
    fn set_paused(&self, paused: bool) -> Result<(), String> {
        let (tx, rx) = mpsc::sync_channel(1);
        self.sender
            .send(Command::Pause(paused, tx))
            .map_err(|_| ENCODER_FAILED)?;
        rx.recv_timeout(Duration::from_secs(10))
            .map_err(|_| ENCODER_FAILED.to_string())
    }
    pub fn pause(&mut self) -> Result<(), String> {
        self.set_paused(true)
    }
    pub fn resume(&mut self) -> Result<(), String> {
        self.set_paused(false)
    }
    pub fn stop(&mut self) -> Result<RecordResult, String> {
        let _ = self.sender.send(Command::Stop);
        let result = self
            .worker
            .take()
            .ok_or(ENCODER_FAILED)?
            .join()
            .map_err(|_| ENCODER_FAILED)?;
        match result {
            Ok(duration_ms) => Ok(RecordResult {
                path: self.options.path.to_string_lossy().into_owned(),
                width: self.options.width,
                height: self.options.height,
                fps: self.options.fps,
                duration_ms,
                size_bytes: std::fs::metadata(&self.options.path)
                    .map_err(|_| DISK_FAILED)?
                    .len(),
                clipboard_copied: false,
            }),
            Err(err) => {
                let _ = std::fs::remove_file(&self.options.path);
                Err(err)
            }
        }
    }
    pub fn cancel(&mut self) -> Result<(), String> {
        let _ = self.sender.send(Command::Cancel);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        match std::fs::remove_file(&self.options.path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(DISK_FAILED.into()),
        }
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        if self.worker.is_some() {
            let _ = self.cancel();
        }
    }
}

struct Runtime;
impl Runtime {
    unsafe fn new() -> WinResult<Self> {
        RoInitialize(RO_INIT_MULTITHREADED)?;
        if let Err(err) = MFStartup(MF_VERSION, MFSTARTUP_FULL) {
            RoUninitialize();
            return Err(err);
        }
        Ok(Self)
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        unsafe {
            let _ = MFShutdown();
            RoUninitialize();
        }
    }
}

struct Clock {
    frequency: i64,
}
impl Clock {
    unsafe fn new() -> WinResult<Self> {
        let mut frequency = 0;
        QueryPerformanceFrequency(&mut frequency)?;
        Ok(Self { frequency })
    }
    unsafe fn now(&self) -> WinResult<i64> {
        let mut value = 0;
        QueryPerformanceCounter(&mut value)?;
        Ok((value as i128 * TICKS as i128 / self.frequency as i128) as i64)
    }
}

struct Capture {
    session: GraphicsCaptureSession,
    pool: Direct3D11CaptureFramePool,
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    crop: ID3D11Texture2D,
    output: ID3D11Texture2D,
    staging: ID3D11Texture2D,
    video: ID3D11VideoContext,
    processor: ID3D11VideoProcessor,
    input_view: ID3D11VideoProcessorInputView,
    output_view: ID3D11VideoProcessorOutputView,
    region: D3D11_BOX,
    width: u32,
    height: u32,
    source_size: windows::Graphics::SizeInt32,
}

unsafe fn texture(
    device: &ID3D11Device,
    width: u32,
    height: u32,
    format: DXGI_FORMAT,
    staging: bool,
) -> WinResult<ID3D11Texture2D> {
    let desc = D3D11_TEXTURE2D_DESC {
        Width: width,
        Height: height,
        MipLevels: 1,
        ArraySize: 1,
        Format: format,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: if staging {
            D3D11_USAGE_STAGING
        } else {
            D3D11_USAGE_DEFAULT
        },
        BindFlags: if staging {
            0
        } else {
            D3D11_BIND_RENDER_TARGET.0 as u32
        },
        CPUAccessFlags: if staging {
            D3D11_CPU_ACCESS_READ.0 as u32
        } else {
            0
        },
        MiscFlags: 0,
    };
    let mut result = None;
    device.CreateTexture2D(&desc, None, Some(&mut result))?;
    result.ok_or_else(windows::core::Error::from_win32)
}

impl Capture {
    unsafe fn new(options: &RecordOptions) -> WinResult<Self> {
        let mut device = None;
        let mut context = None;
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT | D3D11_CREATE_DEVICE_VIDEO_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )?;
        let device = device.unwrap();
        let context = context.unwrap();
        let dxgi: IDXGIDevice = device.cast()?;
        let direct: IDirect3DDevice = CreateDirect3D11DeviceFromDXGIDevice(&dxgi)?.cast()?;
        let monitor = MonitorFromPoint(
            POINT {
                x: options.target.x + 1,
                y: options.target.y + 1,
            },
            MONITOR_DEFAULTTONULL,
        );
        let interop: IGraphicsCaptureItemInterop =
            factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()?;
        let item: GraphicsCaptureItem = interop.CreateForMonitor(monitor)?;
        let source_size = item.Size()?;
        if source_size.Width != options.target.width as i32
            || source_size.Height != options.target.height as i32
        {
            return Err(windows::core::Error::new(
                windows::core::HRESULT(0x80070057u32 as i32),
                "monitor size changed",
            ));
        }
        let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &direct,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            2,
            source_size,
        )?;
        pool.FrameArrived(&TypedEventHandler::new(|_, _| Ok(())))?;
        let session = pool.CreateCaptureSession(&item)?;
        // These properties are absent on older Windows builds. Capture remains supported.
        if let Err(err) = session.SetIsCursorCaptureEnabled(options.cursor) {
            if !options.cursor {
                return Err(err);
            }
        }
        let _ = session.SetIsBorderRequired(false);
        let crop = texture(
            &device,
            options.region.width,
            options.region.height,
            DXGI_FORMAT_B8G8R8A8_UNORM,
            false,
        )?;
        let output = texture(
            &device,
            options.width,
            options.height,
            DXGI_FORMAT_NV12,
            false,
        )?;
        let staging = texture(
            &device,
            options.width,
            options.height,
            DXGI_FORMAT_NV12,
            true,
        )?;
        let video_device: ID3D11VideoDevice = device.cast()?;
        let video: ID3D11VideoContext = context.cast()?;
        let desc = D3D11_VIDEO_PROCESSOR_CONTENT_DESC {
            InputFrameFormat: D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE,
            InputFrameRate: DXGI_RATIONAL {
                Numerator: options.fps,
                Denominator: 1,
            },
            InputWidth: options.region.width,
            InputHeight: options.region.height,
            OutputFrameRate: DXGI_RATIONAL {
                Numerator: options.fps,
                Denominator: 1,
            },
            OutputWidth: options.width,
            OutputHeight: options.height,
            Usage: D3D11_VIDEO_USAGE_PLAYBACK_NORMAL,
        };
        let enumerator = video_device.CreateVideoProcessorEnumerator(&desc)?;
        let processor = video_device.CreateVideoProcessor(&enumerator, 0)?;
        let input_desc = D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC {
            FourCC: 0,
            ViewDimension: D3D11_VPIV_DIMENSION_TEXTURE2D,
            Anonymous: D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC_0 {
                Texture2D: D3D11_TEX2D_VPIV {
                    MipSlice: 0,
                    ArraySlice: 0,
                },
            },
        };
        let output_desc = D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC {
            ViewDimension: D3D11_VPOV_DIMENSION_TEXTURE2D,
            Anonymous: D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC_0 {
                Texture2D: D3D11_TEX2D_VPOV { MipSlice: 0 },
            },
        };
        let mut input_view = None;
        let mut output_view = None;
        video_device.CreateVideoProcessorInputView(
            &crop,
            &enumerator,
            &input_desc,
            Some(&mut input_view),
        )?;
        video_device.CreateVideoProcessorOutputView(
            &output,
            &enumerator,
            &output_desc,
            Some(&mut output_view),
        )?;
        let src = RECT {
            left: 0,
            top: 0,
            right: options.region.width as i32,
            bottom: options.region.height as i32,
        };
        let dst = RECT {
            left: 0,
            top: 0,
            right: options.width as i32,
            bottom: options.height as i32,
        };
        video.VideoProcessorSetStreamSourceRect(&processor, 0, true, Some(&src));
        video.VideoProcessorSetStreamDestRect(&processor, 0, true, Some(&dst));
        video.VideoProcessorSetOutputTargetRect(&processor, true, Some(&dst));
        video.VideoProcessorSetStreamAutoProcessingMode(&processor, 0, false);
        // Full-range RGB input to limited-range BT.709 NV12.
        video.VideoProcessorSetStreamColorSpace(
            &processor,
            0,
            &D3D11_VIDEO_PROCESSOR_COLOR_SPACE { _bitfield: 0 },
        );
        video.VideoProcessorSetOutputColorSpace(
            &processor,
            &D3D11_VIDEO_PROCESSOR_COLOR_SPACE { _bitfield: 1 << 2 },
        );
        session.StartCapture()?;
        Ok(Self {
            session,
            pool,
            device,
            context,
            crop,
            output,
            staging,
            video,
            processor,
            input_view: input_view.unwrap(),
            output_view: output_view.unwrap(),
            region: D3D11_BOX {
                left: options.region.x,
                top: options.region.y,
                front: 0,
                right: options.region.x + options.region.width,
                bottom: options.region.y + options.region.height,
                back: 1,
            },
            width: options.width,
            height: options.height,
            source_size,
        })
    }

    unsafe fn read(&self) -> WinResult<Option<(Vec<u8>, i64)>> {
        let mut timestamp = None;
        while let Ok(frame) = self.pool.TryGetNextFrame() {
            let size = frame.ContentSize()?;
            if size != self.source_size {
                let _ = frame.Close();
                return Err(windows::core::Error::new(
                    windows::core::HRESULT(0x80070057u32 as i32),
                    "display resized during recording",
                ));
            }
            let access: IDirect3DDxgiInterfaceAccess = frame.Surface()?.cast()?;
            let surface: ID3D11Texture2D = access.GetInterface()?;
            self.context.CopySubresourceRegion(
                &self.crop,
                0,
                0,
                0,
                0,
                &surface,
                0,
                Some(&self.region),
            );
            timestamp = Some(frame.SystemRelativeTime()?.Duration);
            frame.Close()?;
        }
        let Some(timestamp) = timestamp else {
            return Ok(None);
        };
        let mut stream = D3D11_VIDEO_PROCESSOR_STREAM {
            Enable: BOOL(1),
            pInputSurface: ManuallyDrop::new(Some(self.input_view.clone())),
            ..Default::default()
        };
        let result = self.video.VideoProcessorBlt(
            &self.processor,
            &self.output_view,
            0,
            std::slice::from_ref(&stream),
        );
        ManuallyDrop::drop(&mut stream.pInputSurface);
        result?;
        self.context.CopyResource(&self.staging, &self.output);
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        self.context
            .Map(&self.staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))?;
        let rows = self.height as usize * 3 / 2;
        let mut bytes = vec![0u8; self.width as usize * rows];
        for row in 0..rows {
            std::ptr::copy_nonoverlapping(
                (mapped.pData as *const u8).add(row * mapped.RowPitch as usize),
                bytes.as_mut_ptr().add(row * self.width as usize),
                self.width as usize,
            );
        }
        self.context.Unmap(&self.staging, 0);
        self.device.GetDeviceRemovedReason()?;
        Ok(Some((bytes, timestamp)))
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        let _ = self.session.Close();
        let _ = self.pool.Close();
    }
}

struct Encoder {
    writer: IMFSinkWriter,
    video: u32,
    audio: Option<u32>,
}
impl Encoder {
    unsafe fn new(options: &RecordOptions) -> WinResult<Self> {
        let mut attributes = None;
        MFCreateAttributes(&mut attributes, 4)?;
        let attributes = attributes.unwrap();
        attributes.SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1)?;
        attributes.SetUINT32(&MF_SINK_WRITER_DISABLE_THROTTLING, 1)?;
        attributes.SetGUID(&MF_TRANSCODE_CONTAINERTYPE, &MFTranscodeContainerType_MPEG4)?;
        let path = HSTRING::from(options.path.as_os_str());
        let writer = MFCreateSinkWriterFromURL(&path, None, &attributes)?;
        let output = MFCreateMediaType()?;
        output.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        output.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264)?;
        output.SetUINT32(&MF_MT_AVG_BITRATE, options.bitrate)?;
        output.SetUINT32(&MF_MT_MPEG2_PROFILE, 100)?;
        video_attributes(&output, options)?;
        let video = writer.AddStream(&output)?;
        let input = MFCreateMediaType()?;
        input.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        input.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12)?;
        video_attributes(&input, options)?;
        writer.SetInputMediaType(video, &input, None)?;
        let audio = if options.audio {
            let output = audio_type(true)?;
            let index = writer.AddStream(&output)?;
            writer.SetInputMediaType(index, &audio_type(false)?, None)?;
            Some(index)
        } else {
            None
        };
        writer.BeginWriting()?;
        Ok(Self {
            writer,
            video,
            audio,
        })
    }
    unsafe fn write(&self, stream: u32, bytes: &[u8], time: i64, duration: i64) -> WinResult<()> {
        let buffer = MFCreateMemoryBuffer(bytes.len() as u32)?;
        let mut data = std::ptr::null_mut();
        buffer.Lock(&mut data, None, None)?;
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), data, bytes.len());
        buffer.Unlock()?;
        buffer.SetCurrentLength(bytes.len() as u32)?;
        let sample = MFCreateSample()?;
        sample.AddBuffer(&buffer)?;
        sample.SetSampleTime(time)?;
        sample.SetSampleDuration(duration)?;
        self.writer.WriteSample(stream, &sample)
    }
}

unsafe fn video_attributes(media: &IMFMediaType, options: &RecordOptions) -> WinResult<()> {
    media.SetUINT64(
        &MF_MT_FRAME_SIZE,
        (u64::from(options.width) << 32) | u64::from(options.height),
    )?;
    media.SetUINT64(&MF_MT_FRAME_RATE, (u64::from(options.fps) << 32) | 1)?;
    media.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, (1u64 << 32) | 1)?;
    media.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
    media.SetUINT32(&MF_MT_YUV_MATRIX, MFVideoTransferMatrix_BT709.0 as u32)?;
    media.SetUINT32(&MF_MT_VIDEO_NOMINAL_RANGE, MFNominalRange_16_235.0 as u32)
}
unsafe fn audio_type(aac: bool) -> WinResult<IMFMediaType> {
    let media = MFCreateMediaType()?;
    media.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
    media.SetGUID(
        &MF_MT_SUBTYPE,
        if aac {
            &MFAudioFormat_AAC
        } else {
            &MFAudioFormat_PCM
        },
    )?;
    media.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 2)?;
    media.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, 48_000)?;
    media.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16)?;
    media.SetUINT32(
        &MF_MT_AUDIO_AVG_BYTES_PER_SECOND,
        if aac { 20_000 } else { 192_000 },
    )?;
    media.SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, if aac { 1 } else { 4 })?;
    if aac {
        media.SetUINT32(&MF_MT_AAC_PAYLOAD_TYPE, 0)?;
        media.SetUINT32(&MF_MT_AAC_AUDIO_PROFILE_LEVEL_INDICATION, 0x29)?;
    }
    Ok(media)
}

const AUDIO_RATE: i64 = 48_000;
const AUDIO_REALIGN_FRAMES: i64 = AUDIO_RATE / 20; // 50ms
const AUDIO_IDLE_TICKS: i64 = TICKS / 5; // 200ms

#[derive(Default)]
struct AudioTimeline {
    anchored: bool,
}

impl AudioTimeline {
    // QPC anchors each active interval. Within it the sample clock is the
    // authority: packet timestamp jitter must not insert or remove samples.
    fn place(&mut self, written: i64, at: i64, frames: i64, discontinuity: bool) -> (i64, i64) {
        let drift = at - written;
        if !self.anchored || discontinuity || drift.abs() > AUDIO_REALIGN_FRAMES {
            // Stay unanchored when the whole packet is stale; the next packet
            // must also be checked until its samples reach the current cursor.
            let skip = (-drift).max(0).min(frames);
            self.anchored = skip < frames;
            (written.max(at), skip)
        } else {
            (written, 0)
        }
    }

    fn reset(&mut self) {
        self.anchored = false;
    }
}

struct Loopback {
    client: IAudioClient,
    capture: IAudioCaptureClient,
    written: i64,
    timeline: AudioTimeline,
    last_packet_received: Option<i64>,
}
impl Loopback {
    unsafe fn new() -> WinResult<Self> {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let endpoint = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
        let client: IAudioClient = endpoint.Activate(CLSCTX_ALL, None)?;
        let format = WAVEFORMATEX {
            wFormatTag: 1,
            nChannels: 2,
            nSamplesPerSec: 48_000,
            nAvgBytesPerSec: 192_000,
            nBlockAlign: 4,
            wBitsPerSample: 16,
            cbSize: 0,
        };
        client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_LOOPBACK
                | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM
                | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY,
            1_000_000,
            0,
            &format,
            None,
        )?;
        let capture = client.GetService()?;
        client.Start()?;
        Ok(Self {
            client,
            capture,
            written: 0,
            timeline: AudioTimeline::default(),
            last_packet_received: None,
        })
    }

    unsafe fn silence_until(&mut self, encoder: &Encoder, frame: i64) -> WinResult<()> {
        let stream = encoder.audio.unwrap();
        while self.written < frame {
            let count = (frame - self.written).min(4800);
            encoder.write(
                stream,
                &vec![0; count as usize * 4],
                self.written * TICKS / 48_000,
                count * TICKS / 48_000,
            )?;
            self.written += count;
        }
        Ok(())
    }

    fn resume(&mut self, now: i64) {
        self.timeline.reset();
        self.last_packet_received = Some(now);
    }

    unsafe fn fill_if_idle(
        &mut self,
        encoder: &Encoder,
        origin: i64,
        paused_ticks: i64,
        now: i64,
    ) -> WinResult<()> {
        if now - self.last_packet_received.unwrap_or(origin) > AUDIO_IDLE_TICKS {
            // Preserve 100ms of headroom for in-flight packets, but only enter
            // this path after 200ms with no packet at all (including SILENT).
            let until = ((now - origin - paused_ticks - TICKS / 10).max(0) * AUDIO_RATE) / TICKS;
            self.silence_until(encoder, until)?;
            self.timeline.reset();
        }
        Ok(())
    }

    unsafe fn drain(
        &mut self,
        encoder: &Encoder,
        origin: i64,
        paused_ticks: i64,
        minimum_qpc: i64,
        now: i64,
        paused: bool,
        finishing: bool,
    ) -> WinResult<()> {
        while self.capture.GetNextPacketSize()? > 0 {
            let mut data = std::ptr::null_mut();
            let mut frames = 0;
            let mut flags = 0;
            let mut qpc = 0;
            self.capture
                .GetBuffer(&mut data, &mut frames, &mut flags, None, Some(&mut qpc))?;
            let bytes = if flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 || data.is_null() {
                vec![0; frames as usize * 4]
            } else {
                std::slice::from_raw_parts(data, frames as usize * 4).to_vec()
            };
            self.capture.ReleaseBuffer(frames)?;
            self.last_packet_received = Some(now);
            if paused {
                self.timeline.reset();
                continue;
            }
            let timestamp_valid = flags & AUDCLNT_BUFFERFLAGS_TIMESTAMP_ERROR.0 as u32 == 0;
            // An invalid timestamp cannot establish a new interval. Once
            // anchored, keep its samples continuous instead of dropping them.
            if !timestamp_valid && !self.timeline.anchored {
                continue;
            }
            let qpc = qpc as i64;
            let boundary_skip = if timestamp_valid {
                (((minimum_qpc.max(origin) - qpc).max(0) * AUDIO_RATE + TICKS - 1) / TICKS)
                    .min(frames as i64)
            } else {
                0
            };
            // Only trim the packet tail at the final stop boundary. A normal
            // packet can end after the loop's earlier QPC snapshot.
            let available = if finishing && timestamp_valid {
                (((now - qpc).max(0) * AUDIO_RATE) / TICKS).min(frames as i64)
            } else {
                frames as i64
            };
            let usable = available - boundary_skip;
            if usable <= 0 {
                continue;
            }
            let at = if timestamp_valid {
                ((qpc - origin - paused_ticks) * AUDIO_RATE / TICKS + boundary_skip).max(0)
            } else {
                self.written
            };
            let discontinuity =
                timestamp_valid && flags & AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY.0 as u32 != 0;
            let (position, overlap) = self.timeline.place(self.written, at, usable, discontinuity);
            let count = usable - overlap;
            if count <= 0 {
                continue;
            }
            self.silence_until(encoder, position)?;
            let offset = (boundary_skip + overlap) as usize * 4;
            encoder.write(
                encoder.audio.unwrap(),
                &bytes[offset..offset + count as usize * 4],
                self.written * TICKS / AUDIO_RATE,
                count * TICKS / AUDIO_RATE,
            )?;
            self.written += count;
        }
        Ok(())
    }
}
impl Drop for Loopback {
    fn drop(&mut self) {
        unsafe {
            let _ = self.client.Stop();
        }
    }
}

unsafe fn record(
    options: &RecordOptions,
    commands: mpsc::Receiver<Command>,
    status: &Mutex<RecordStatus>,
    ready: &mpsc::SyncSender<Result<(), String>>,
) -> WinResult<u64> {
    let _runtime = Runtime::new()?;
    let clock = Clock::new()?;
    let capture = Capture::new(options)?;
    let encoder = Encoder::new(options)?;
    let started = clock.now()?;
    let (mut latest, _) = loop {
        if let Some(frame) = capture.read()? {
            break frame;
        }
        if clock.now()? - started > 5 * TICKS {
            return Err(windows::core::Error::new(
                windows::core::HRESULT(0x80004005u32 as i32),
                "WGC delivered no frame",
            ));
        }
        thread::sleep(Duration::from_millis(5));
    };
    let mut audio = if options.audio {
        Some(Loopback::new()?)
    } else {
        None
    };
    let origin = clock.now()?;
    let mut paused_at = None;
    let mut paused_ticks = 0;
    let mut minimum_qpc = origin;
    let mut frames_written = 0i64;
    let _ = ready.send(Ok(()));
    loop {
        if let Some((bytes, timestamp)) = capture.read()? {
            if timestamp >= minimum_qpc && paused_at.is_none() {
                latest = bytes;
            }
        }
        let now = clock.now()?;
        let elapsed = paused_at.unwrap_or(now) - origin - paused_ticks;
        if let Some(audio) = audio.as_mut() {
            if let Some(boundary) = paused_at {
                // WASAPI can deliver a packet after Pause whose leading samples
                // precede Pause. Preserve that prefix; discard every sample in
                // the paused interval, rather than losing an entire packet.
                audio.drain(
                    &encoder,
                    origin,
                    paused_ticks,
                    minimum_qpc,
                    boundary,
                    false,
                    true,
                )?;
            } else {
                audio.drain(
                    &encoder,
                    origin,
                    paused_ticks,
                    minimum_qpc,
                    now,
                    false,
                    false,
                )?;
                audio.fill_if_idle(&encoder, origin, paused_ticks, now)?;
            }
        }
        if paused_at.is_none() && frames_written * TICKS / i64::from(options.fps) <= elapsed {
            let time = frames_written * TICKS / i64::from(options.fps);
            let next = (frames_written + 1) * TICKS / i64::from(options.fps);
            encoder.write(encoder.video, &latest, time, next - time)?;
            frames_written += 1;
        }
        {
            let mut state = status.lock().unwrap();
            state.elapsed_ms = (elapsed.max(0) / 10_000) as u64;
            state.state = if paused_at.is_some() {
                RecordState::Paused
            } else {
                RecordState::Recording
            };
        }
        match commands.recv_timeout(Duration::from_millis(3)) {
            Ok(Command::Pause(pause, reply)) => {
                let now = clock.now()?;
                if pause && paused_at.is_none() {
                    paused_at = Some(now);
                }
                if !pause {
                    if let Some(start) = paused_at.take() {
                        // Drain buffered pause packets before starting a new active interval.
                        if let Some(audio) = audio.as_mut() {
                            audio.drain(
                                &encoder,
                                origin,
                                paused_ticks,
                                minimum_qpc,
                                now,
                                true,
                                false,
                            )?;
                            audio.resume(now);
                        }
                        paused_ticks += now - start;
                        minimum_qpc = now;
                    }
                }
                status.lock().unwrap().state = if pause {
                    RecordState::Paused
                } else {
                    RecordState::Recording
                };
                let _ = reply.send(());
            }
            Ok(Command::Stop) => {
                status.lock().unwrap().state = RecordState::Finalizing;
                let end = paused_at.unwrap_or(clock.now()?);
                let elapsed = (end - origin - paused_ticks).max(0);
                if let Some(audio) = audio.as_mut() {
                    audio.drain(
                        &encoder,
                        origin,
                        paused_ticks,
                        minimum_qpc,
                        end,
                        paused_at.is_some(),
                        true,
                    )?;
                    audio.silence_until(&encoder, elapsed * 48_000 / TICKS)?;
                }
                encoder.writer.Finalize()?;
                return Ok((elapsed / 10_000) as u64);
            }
            Ok(Command::Cancel) | Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(0),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }
}

pub fn copy_file(path: &Path) -> Result<(), String> {
    let owner = crate::APP
        .get()
        .and_then(|app| {
            use tauri::Manager;
            app.get_window("daemon").and_then(|w| w.hwnd().ok())
        })
        .map(|h| windows::Win32::Foundation::HWND(h.0 as *mut _))
        .unwrap_or_default();
    copy_file_with_owner(path, owner)
}

fn copy_file_with_owner(
    path: &Path,
    owner: windows::Win32::Foundation::HWND,
) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::System::DataExchange::*;
    use windows::Win32::System::Memory::*;
    use windows::Win32::UI::Shell::DROPFILES;
    unsafe {
        let names: Vec<u16> = path.as_os_str().encode_wide().chain([0, 0]).collect();
        let memory = GlobalAlloc(GMEM_MOVEABLE, size_of::<DROPFILES>() + names.len() * 2)
            .map_err(failure)?;
        let data = GlobalLock(memory);
        if data.is_null() {
            let _ = windows::Win32::Foundation::GlobalFree(memory);
            return Err("screen_record.clipboard_failed".into());
        }
        let header = DROPFILES {
            pFiles: size_of::<DROPFILES>() as u32,
            fWide: BOOL(1),
            ..Default::default()
        };
        std::ptr::write(data as *mut DROPFILES, header);
        std::ptr::copy_nonoverlapping(
            names.as_ptr() as *const u8,
            (data as *mut u8).add(size_of::<DROPFILES>()),
            names.len() * 2,
        );
        let _ = GlobalUnlock(memory);
        // A non-null owner is required before EmptyClipboard/SetClipboardData.
        let mut opened = false;
        for _ in 0..10 {
            if OpenClipboard(owner).is_ok() {
                opened = true;
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        if !opened {
            let _ = windows::Win32::Foundation::GlobalFree(memory);
            return Err("screen_record.clipboard_failed".into());
        }
        let result =
            EmptyClipboard().and_then(|_| SetClipboardData(15, HANDLE(memory.0)).map(|_| ()));
        let _ = CloseClipboard();
        if result.is_err() {
            let _ = windows::Win32::Foundation::GlobalFree(memory);
        }
        result.map_err(failure)
    }
}

pub fn reveal(path: &Path) -> Result<(), String> {
    use windows::Win32::UI::Shell::{ILCreateFromPathW, ILFree, SHOpenFolderAndSelectItems};
    unsafe {
        let path = HSTRING::from(path.as_os_str());
        let item = ILCreateFromPathW(PCWSTR(path.as_ptr()));
        if item.is_null() {
            return Err("screen_record.no_result".into());
        }
        let result = SHOpenFolderAndSelectItems(item, None, 0);
        ILFree(Some(item));
        result.map_err(failure)
    }
}

#[cfg(test)]
mod audio_tests {
    use super::*;

    #[test]
    fn sample_clock_ignores_packet_timestamp_jitter() {
        let mut timeline = AudioTimeline::default();
        assert_eq!(timeline.place(0, 120, 480, false), (120, 0));
        for jitter in [-12, -1, 0, 1, 12, 2400] {
            assert_eq!(timeline.place(600, 600 + jitter, 480, false), (600, 0));
        }
    }

    #[test]
    fn real_gaps_and_resume_reanchor_without_reversing_time() {
        let mut timeline = AudioTimeline { anchored: true };
        assert_eq!(timeline.place(4800, 8000, 480, false), (8000, 0));
        assert_eq!(timeline.place(8000, 4000, 480, false), (8000, 480));
        assert_eq!(timeline.place(8000, 7800, 480, false), (8000, 200));
        assert_eq!(timeline.place(8000, 7990, 480, true), (8000, 10));
        timeline.reset();
        assert_eq!(timeline.place(8000, 8012, 480, false), (8012, 0));
    }
}
