use crate::{
    data::{
        awg::{AwgChannelDisplay, AwgConfig},
        head::{Channel, ChannelDisplay, DataHead},
        units::{Frequency, Voltage},
    },
    InitialDeviceRunConfig, Measurements, OscilloscopeCommand, OscilloscopeData,
    OscilloscopeMessage, OscilloscopeRunCommand, OscilloscopeRunSetting, SignalData,
};
use arrayvec::ArrayVec;
use snafu::{ensure, Location, OptionExt, ResultExt, Snafu};
use std::{
    io::Write,
    str::{from_utf8, Utf8Error},
    time::Duration,
};
use tokio::{
    sync::mpsc::{self, error::TryRecvError},
    time::Instant,
};
use windows::{
    core::HSTRING,
    Devices::{
        Enumeration::DeviceInformation,
        Usb::{UsbBulkInPipe, UsbBulkOutPipe, UsbDevice, UsbInterface, UsbWriteOptions},
    },
    Storage::Streams::{DataReader, DataWriter},
};

pub const VID: u32 = 0x5345;
pub const PID: u32 = 0x1234;

#[derive(Debug, Snafu)]
pub enum FromUsbDeviceError {
    WrongVidPid {
        vid: u32,
        pid: u32,
    },
    #[snafu(context(false))]
    Windows {
        source: WindowsError,
    },
    #[snafu(context(false))]
    DeviceInitialization {
        source: DeviceInitializationError,
    },
}

impl From<windows::core::Error> for FromUsbDeviceError {
    #[track_caller]
    fn from(source: windows::core::Error) -> Self {
        FromUsbDeviceError::Windows {
            source: WindowsError::from(source),
        }
    }
}

#[derive(Debug, Snafu)]
pub enum DeviceInitializationError {
    #[snafu(context(false))]
    Windows {
        source: WindowsError,
    },
    BulkInPipeNotFound,
    BulkOutPipeNotFound,
}

impl From<windows::core::Error> for DeviceInitializationError {
    #[track_caller]
    fn from(source: windows::core::Error) -> Self {
        DeviceInitializationError::Windows {
            source: WindowsError::from(source),
        }
    }
}

#[derive(Debug, Snafu)]
#[snafu(context(false))]
pub struct WindowsError {
    #[snafu(implicit)]
    location: Location,
    source: windows::core::Error,
}

#[allow(dead_code)]
pub struct Device {
    device: UsbDevice,
    interface: UsbInterface,
    bulk_in: UsbBulkInPipe,
    bulk_out: UsbBulkOutPipe,
}

impl Device {
    pub async fn from_first_vid_pid_match() -> Result<Self, FromUsbDeviceError> {
        let selector = UsbDevice::GetDeviceSelectorVidPidOnly(VID, PID)?;

        let device = DeviceInformation::FindAllAsyncAqsFilter(&selector)?
            .await?
            .into_iter()
            .next()
            .expect("no device found");

        let device = UsbDevice::FromIdAsync(&device.Id()?)?.await?;

        Self::from_usb_device(device)
    }

    pub async fn from_device_id(device_id: impl Into<HSTRING>) -> Result<Self, FromUsbDeviceError> {
        let device = UsbDevice::FromIdAsync(&device_id.into())?.await?;

        Self::from_usb_device(device)
    }

    pub fn blocking_from_device_id(
        device_id: impl Into<HSTRING>,
    ) -> Result<Self, FromUsbDeviceError> {
        let device = UsbDevice::FromIdAsync(&device_id.into())?.get()?;

        Self::from_usb_device(device)
    }

    pub fn from_usb_device(device: UsbDevice) -> Result<Self, FromUsbDeviceError> {
        let descriptor = device.DeviceDescriptor()?;
        let vid = descriptor.VendorId()?;
        let pid = descriptor.ProductId()?;

        ensure!(vid == VID && pid == PID, WrongVidPidSnafu { vid, pid });

        Ok(Self::initialize_device(device)?)
    }

    fn initialize_device(device: UsbDevice) -> Result<Self, DeviceInitializationError> {
        let interface = device.DefaultInterface()?;

        let bulk_in = interface
            .BulkInPipes()?
            .into_iter()
            .find(|p| {
                matches!(
                    p.EndpointDescriptor().and_then(|ed| ed.EndpointNumber()),
                    Ok(1)
                )
            })
            .context(BulkInPipeNotFoundSnafu)?;

        let bulk_out = interface
            .BulkOutPipes()?
            .into_iter()
            .find(|p| {
                matches!(
                    p.EndpointDescriptor().and_then(|ed| ed.EndpointNumber()),
                    Ok(1)
                )
            })
            .context(BulkOutPipeNotFoundSnafu)?;

        bulk_out.SetWriteOptions(UsbWriteOptions::AutoClearStall)?;

        Ok(Self {
            device,
            interface,
            bulk_in,
            bulk_out,
        })
    }

    pub fn raw_io(&self) -> Result<Io, WindowsError> {
        let input = self.bulk_in.InputStream()?;
        let output = self.bulk_out.OutputStream()?;
        Ok(Io {
            r: DataReader::CreateDataReader(&input)?,
            w: DataWriter::CreateDataWriter(&output)?,
            last_write: Instant::now(),
        })
    }
}

pub struct Io {
    r: DataReader,
    w: DataWriter,
    last_write: Instant,
}

impl Io {
    pub async fn cmd(&mut self, command: &[u8]) -> Result<(), WindowsError> {
        const MIN_PAUSE: Duration = Duration::from_millis(10);

        if let Some(wait) = MIN_PAUSE.checked_sub(self.last_write.elapsed()) {
            tokio::time::sleep(wait).await;
        }

        self.cmd_nowait(command).await
    }

    pub async fn cmd_nowait(&mut self, command: &[u8]) -> Result<(), WindowsError> {
        self.w.WriteBytes(command)?;
        self.w.StoreAsync()?.await?;

        self.last_write = Instant::now();

        Ok(())
    }

    pub async fn cmd_with_writer<'a>(
        &'a mut self,
        f: impl FnOnce(&mut IoWriter<'a>) -> Result<(), std::io::Error>,
    ) -> Result<(), IoError> {
        let mut io_writer = IoWriter(&self.w);
        f(&mut io_writer)?;

        const MIN_PAUSE: Duration = Duration::from_millis(10);

        if let Some(wait) = MIN_PAUSE.checked_sub(self.last_write.elapsed()) {
            tokio::time::sleep(wait).await;
        }
        self.w.StoreAsync()?.await?;

        self.last_write = Instant::now();
        Ok(())
    }

    pub async fn read<'a>(&mut self, buf: &'a mut [u8]) -> Result<&'a mut [u8], WindowsError> {
        assert!(buf.len() <= u32::MAX as usize);

        let bytes_read = self.r.LoadAsync(buf.len() as u32)?.await?;
        let buf_len = buf.len();

        let sliced_buf = &mut buf[..buf_len.min(bytes_read as usize)];
        self.r.ReadBytes(sliced_buf)?;

        Ok(sliced_buf)
    }

    pub async fn cmd_output<'b>(
        &mut self,
        command: &[u8],
        buf: &'b mut [u8],
    ) -> Result<&'b mut [u8], WindowsError> {
        self.cmd(command).await?;
        self.read(buf).await
    }
}

pub struct IoWriter<'a>(&'a DataWriter);

#[derive(Debug, Snafu)]
pub enum IoError {
    #[snafu(context(false))]
    Windows { source: WindowsError },
    #[snafu(context(false))]
    Io { source: std::io::Error },
}

impl From<windows::core::Error> for IoError {
    #[track_caller]
    fn from(source: windows::core::Error) -> Self {
        IoError::Windows {
            source: WindowsError::from(source),
        }
    }
}

impl<'a> Write for IoWriter<'a> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0
            .WriteBytes(buf)
            .map(|_| buf.len())
            .map_err(|e| std::io::Error::from_raw_os_error(e.code().0))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub async fn run_device_loop(
    device: Device,
    message_tx: mpsc::Sender<OscilloscopeMessage>,
    mut commands_rx: mpsc::Receiver<OscilloscopeRunCommand>,
    initial_config: InitialDeviceRunConfig,
    mut notify_updated: impl FnMut(),
) -> Result<(), RunError> {
    let mut io = device.raw_io()?;

    let mut ch0_enabled = true;
    let mut ch1_enabled = true;
    let mut measurements_enabled = initial_config.measurements_enabled;

    'main: loop {
        'commands: loop {
            let cmd = match commands_rx.try_recv() {
                Err(TryRecvError::Disconnected) => break 'main,
                Err(TryRecvError::Empty) => break 'commands,
                Ok(cmd) => cmd,
            };

            match cmd {
                OscilloscopeRunCommand::Command(c) => {
                    send_command(c, &mut io).await?;
                }
                OscilloscopeRunCommand::RunSetting(setting) => match setting {
                    OscilloscopeRunSetting::SetMeasurementsEnabled(measurements) => {
                        measurements_enabled = measurements;
                    }
                    OscilloscopeRunSetting::ReadAwgConfig => {
                        let config = read_awg_config(&mut io).await?;
                        if message_tx
                            .send(OscilloscopeMessage::Awg(config))
                            .await
                            .is_err()
                        {
                            break 'main;
                        }
                    }
                    OscilloscopeRunSetting::SetAwgConfig(config) => {
                        set_awg_config(&mut io, config).await?;
                        let config = read_awg_config(&mut io).await?;
                        if message_tx
                            .send(OscilloscopeMessage::Awg(config))
                            .await
                            .is_err()
                        {
                            break 'main;
                        }
                    }
                },
            }
        }

        let i = Instant::now();

        let signal_data = get_signal(&mut io, ch0_enabled, ch1_enabled).await?;
        ch0_enabled = signal_data.head.channels[0].display == ChannelDisplay::On;
        ch1_enabled = signal_data.head.channels[1].display == ChannelDisplay::On;

        let measurements = if measurements_enabled {
            let ch0_measurements = if measurements_enabled && ch0_enabled {
                get_measurements(&mut io, Channel::Ch1).await?
            } else {
                Default::default()
            };

            let ch1_measurements = if measurements_enabled && ch1_enabled {
                get_measurements(&mut io, Channel::Ch2).await?
            } else {
                Default::default()
            };

            Some([ch0_measurements, ch1_measurements])
        } else {
            None
        };

        let elapsed = i.elapsed();

        let data = OscilloscopeData {
            measurements,
            signal_data,
            acquisition_duration: elapsed,
        };

        notify_updated();
        if message_tx
            .send(OscilloscopeMessage::Data(data))
            .await
            .is_err()
        {
            break 'main;
        }
    }

    Ok(())
}

async fn send_command(cmd: OscilloscopeCommand, io: &mut Io) -> Result<(), RunError> {
    let buf = &mut [0u8; 8 * 1024];
    match cmd {
        OscilloscopeCommand::SetHorizontalOffset(offset) => {
            // HACK: 0.0001 because the rounding/float parsing on the device is a bit wonky
            let offset = offset + offset.signum() * 0.0001;
            io.cmd_with_writer(|w| write!(w, ":HORIzontal:OFFSet {offset:.4}"))
                .await?;
        }
        OscilloscopeCommand::SetChannelDisplay(channel, enabled) => {
            io.cmd_with_writer(|w| write!(w, ":{channel}:DISPlay {enabled}"))
                .await?;
        }
        OscilloscopeCommand::SetChannelVOffset(channel, offset) => {
            // HACK: 0.0001 because the rounding/float parsing on the device is a bit wonky
            let offset = offset + offset.signum() * 0.0001;
            io.cmd_with_writer(|w| write!(w, ":{channel}:OFFSet {offset:.4}"))
                .await?;
        }
        OscilloscopeCommand::SetChannelVScale(channel, scale) => {
            io.cmd_with_writer(|w| write!(w, ":{channel}:SCALe {scale:.2}"))
                .await?;
            // make sure the device is ready again
            io.cmd_with_writer(|w| write!(w, ":{channel}:SCALe?"))
                .await?;
            let _ = io.read(buf).await?;
        }
        OscilloscopeCommand::SetChannelCoupling(channel, coupling) => {
            io.cmd_with_writer(|w| write!(w, ":{channel}:COUPling {coupling}"))
                .await?;
        }
        OscilloscopeCommand::SetChannelAttenuation(channel, att) => {
            io.cmd_with_writer(|w| write!(w, ":{channel}:PROBe {att}"))
                .await?;
        }
        OscilloscopeCommand::SetTimeScale(time) => {
            io.cmd_with_writer(|w| write!(w, ":HORIzontal:SCALe {time:#}"))
                .await?;
            // make sure the device is ready again
            let _ = io.cmd_output(b":HORIzontal:SCALe?", buf).await?;
        }
        OscilloscopeCommand::SetTriggerSource(channel) => {
            io.cmd_with_writer(|w| write!(w, ":TRIGger:SINGle:SOURce {channel}"))
                .await?;
        }
        OscilloscopeCommand::SetTriggerEdge(edge) => {
            io.cmd_with_writer(|w| write!(w, ":TRIGger:SINGle:EDGe {edge}"))
                .await?;
        }
        OscilloscopeCommand::SetTriggerLevel(voltage) => {
            // HACK: 0.0001 because the rounding/float parsing on the device is a bit wonky
            let voltage = Voltage(voltage.0 + voltage.0.signum() * 0.0001);
            io.cmd_with_writer(|w| write!(w, ":TRIGger:SINGle:EDGe:LEVel {voltage}"))
                .await?;
        }
        OscilloscopeCommand::SetTriggerSweep(sweep) => {
            io.cmd_with_writer(|w| write!(w, ":TRIGger:SINGle:SWEep {sweep}"))
                .await?;
        }
        OscilloscopeCommand::SetTriggerCoupling(coupling) => {
            io.cmd_with_writer(|w| write!(w, ":TRIGger:SINGle:COUPling {coupling}"))
                .await?;
        }
        OscilloscopeCommand::SetAcquisitionMode(ty) => {
            io.cmd_with_writer(|w| write!(w, ":ACQuire:MODe {ty}"))
                .await?;
        }
        OscilloscopeCommand::SetAcquisitionDepth(d) => {
            io.cmd_with_writer(|w| write!(w, ":ACQuire:DEPMem {d}"))
                .await?;
        }
        OscilloscopeCommand::Auto => {
            io.cmd(b":AUToset .").await?;
        }
    }

    Ok(())
}

async fn read_awg_config(io: &mut Io) -> Result<AwgConfig, RunError> {
    let buf = &mut [0u8; 1024];

    Ok(AwgConfig {
        enabled: {
            let enabled = io.cmd_output(b":CHAN?", buf).await?;
            from_utf8(enabled)?
                .trim()
                .parse::<AwgChannelDisplay>()?
                .into()
        },
        mode: {
            let mode = io.cmd_output(b":FUNC?", buf).await?;
            from_utf8(mode)?.trim().parse()?
        },
        frequency: {
            let freq = io.cmd_output(b":FUNC:FREQ?", buf).await?;
            // BUG: frequency readout is micro-Hz for some reason
            Frequency(from_utf8(freq)?.trim().parse::<f64>()? / 1e6)
        },
        amplitude: {
            let volt = io.cmd_output(b":FUNC:AMPL?", buf).await?;
            // BUG: amplitude readout is millivolt for some reason
            Voltage(from_utf8(volt)?.trim().parse::<f64>()? / 1e3)
        },
        offset: {
            let volt = io.cmd_output(b":FUNC:OFFS?", buf).await?;
            // BUG: offset readout is millivolt for some reason
            Voltage(from_utf8(volt)?.trim().parse::<f64>()? / 1e3)
        },
    })
}

async fn set_awg_config(io: &mut Io, config: AwgConfig) -> Result<(), RunError> {
    io.cmd_with_writer(|w| write!(w, ":FUNC {}", config.mode))
        .await?;
    io.cmd_with_writer(|w| write!(w, ":FUNC:FREQ {}", config.frequency.0))
        .await?;
    io.cmd_with_writer(|w| write!(w, ":FUNC:AMPL {}", config.amplitude.0))
        .await?;
    io.cmd_with_writer(|w| write!(w, ":FUNC:OFFS {}", config.offset.0))
        .await?;
    io.cmd_with_writer(|w| write!(w, ":CHAN {}", AwgChannelDisplay::from(config.enabled)))
        .await?;

    Ok(())
}

async fn get_signal(
    io: &mut Io,
    ch0_enabled: bool,
    ch1_enabled: bool,
) -> Result<SignalData, RunError> {
    let buf = &mut [0u8; 1024 + 4];
    let buf2 = &mut [0u8; 1024 + 4];
    let buf3 = &mut [0u8; 1024 + 4];

    let should_read_data = ch0_enabled || ch1_enabled;

    if should_read_data {
        if ch0_enabled {
            io.cmd_nowait(b":DATa:WAVe:SCReen:CH1?").await?;
        } else {
            io.cmd_nowait(b":DATa:WAVe:SCReen:CH2?").await?;
        }
    }
    io.cmd_nowait(b":DATa:WAVe:SCReen:HEAD?").await?;

    let read1 = io.read(buf).await?;
    let (head, ch_data): (DataHead, _) = if should_read_data {
        let read2 = io.read(buf2).await?;

        match serde_json::from_slice(&read2[4..]) {
            Ok(head) => (head, Some(read1)),
            Err(e) => (
                serde_json::from_slice(&read1[4..]).context(JsonSnafu { source2: Some(e) })?,
                Some(read2),
            ),
        }
    } else {
        (
            serde_json::from_slice(&read1[4..]).context(JsonSnafu { source2: None })?,
            None,
        )
    };

    let ch_vec = ch_data.map(|d| {
        let mut ch_vec = ArrayVec::new();
        ch_vec.extend(d[4..].iter().copied());
        ch_vec
    });

    let ch_vec_2 = if should_read_data && ch1_enabled {
        io.cmd_nowait(b":DATa:WAVe:SCReen:CH2?").await?;
        let read3 = io.read(buf3).await?;

        let mut ch_vec = ArrayVec::new();
        ch_vec.extend(read3[4..].iter().copied());
        Some(ch_vec)
    } else {
        None
    };

    let (ch_vec, ch_vec_2) = match (ch0_enabled, ch1_enabled) {
        (true, true) => (ch_vec, ch_vec_2),
        (true, false) => (ch_vec, None),
        (false, true) => (None, ch_vec),
        (false, false) => (None, None),
    };

    Ok(SignalData {
        head,
        ch0_data: ch_vec,
        ch1_data: ch_vec_2,
    })
}

async fn get_measurements(io: &mut Io, ch: Channel) -> Result<Measurements, RunError> {
    let commands = Measurements::channel_to_measurement_commands(ch);

    let mut measurements = Measurements::default();
    let buf = &mut [0u8; 64];
    for cmd in commands {
        io.cmd_nowait(cmd).await?;
        let read = io.read(buf).await?;
        measurements.with_parsed(std::str::from_utf8(read)?);
    }

    Ok(measurements)
}

#[derive(Debug, Snafu)]
pub enum RunError {
    #[snafu(context(false))]
    Io { source: IoError },
    Json {
        source: serde_json::Error,
        source2: Option<serde_json::Error>,
    },
    #[snafu(context(false))]
    Utf8 { source: Utf8Error },
    #[snafu(context(false))]
    Strum { source: strum::ParseError },
    #[snafu(context(false))]
    Float { source: std::num::ParseFloatError },
}

impl From<WindowsError> for RunError {
    #[track_caller]
    fn from(source: WindowsError) -> Self {
        Self::Io {
            source: IoError::Windows { source },
        }
    }
}
