use crate::{
    data::{
        awg::{AwgChannelDisplay, AwgConfig},
        head::{Channel, ChannelDisplay, DataHeader},
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
    time::{error::Elapsed, timeout, Instant},
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

pub const IO_TIMEOUT: Duration = Duration::from_secs(2);
const MIN_PAUSE: Duration = Duration::from_millis(10);

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
    pub async fn send(&mut self, command: &[u8]) -> Result<(), IoError> {
        timeout(IO_TIMEOUT, self.raw_send(command)).await?
    }

    pub async fn raw_send(&mut self, command: &[u8]) -> Result<(), IoError> {
        if let Some(wait) = MIN_PAUSE.checked_sub(self.last_write.elapsed()) {
            tokio::time::sleep(wait).await;
        }

        self.raw_send_nowait(command).await
    }

    pub async fn send_with_writer<'a>(
        &'a mut self,
        f: impl FnOnce(&mut IoWriter<'a>) -> Result<(), std::io::Error>,
    ) -> Result<(), IoError> {
        timeout(IO_TIMEOUT, self.raw_send_with_writer(f)).await?
    }

    pub async fn recv<'a>(&mut self, buf: &'a mut [u8]) -> Result<&'a mut [u8], IoError> {
        timeout(IO_TIMEOUT, self.raw_recv(buf)).await?
    }

    pub async fn send_with_output<'b>(
        &mut self,
        command: &[u8],
        buf: &'b mut [u8],
    ) -> Result<&'b mut [u8], IoError> {
        self.send(command).await?;
        self.recv(buf).await
    }

    pub async fn raw_recv<'a>(&mut self, buf: &'a mut [u8]) -> Result<&'a mut [u8], IoError> {
        assert!(buf.len() <= u32::MAX as usize);

        let bytes_read = self.r.LoadAsync(buf.len() as u32)?.await?;
        let buf_len = buf.len();

        let sliced_buf = &mut buf[..buf_len.min(bytes_read as usize)];
        self.r.ReadBytes(sliced_buf)?;

        Ok(sliced_buf)
    }

    pub async fn raw_send_nowait(&mut self, command: &[u8]) -> Result<(), IoError> {
        self.w.WriteBytes(command)?;

        self.last_write = Instant::now();
        self.w.StoreAsync()?.await?;

        Ok(())
    }

    pub async fn raw_send_with_writer<'a>(
        &'a mut self,
        f: impl FnOnce(&mut IoWriter<'a>) -> Result<(), std::io::Error>,
    ) -> Result<(), IoError> {
        let mut io_writer = IoWriter(&self.w);
        f(&mut io_writer)?;

        if let Some(wait) = MIN_PAUSE.checked_sub(self.last_write.elapsed()) {
            tokio::time::sleep(wait).await;
        }

        self.last_write = Instant::now();
        self.w.StoreAsync()?.await?;

        Ok(())
    }
}

pub struct IoWriter<'a>(&'a DataWriter);

#[derive(Debug, Snafu)]
pub enum IoError {
    #[snafu(transparent)]
    Windows { source: WindowsError },
    #[snafu(context(false))]
    Io { source: std::io::Error },
    #[snafu(context(false))]
    Timeout { source: Elapsed },
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
    let mut io = device.raw_io().context(IoOpenSnafu)?;

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
        ch0_enabled = signal_data.header.channels[0].display == ChannelDisplay::On;
        ch1_enabled = signal_data.header.channels[1].display == ChannelDisplay::On;

        let measurements = if measurements_enabled {
            let ch0_measurements = if measurements_enabled && ch0_enabled {
                get_measurements(&mut io, Channel::Ch1)
                    .await
                    .context(AcquireMeasurementSnafu {
                        channel: Channel::Ch1,
                    })?
            } else {
                Default::default()
            };

            let ch1_measurements = if measurements_enabled && ch1_enabled {
                get_measurements(&mut io, Channel::Ch2)
                    .await
                    .context(AcquireMeasurementSnafu {
                        channel: Channel::Ch2,
                    })?
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
            io.send_with_writer(|w| write!(w, ":HORIzontal:OFFSet {offset:.4}"))
                .await
                .context(SetHorizontalOffsetSnafu)?;
        }
        OscilloscopeCommand::SetChannelDisplay(channel, enabled) => {
            io.send_with_writer(|w| write!(w, ":{channel}:DISPlay {enabled}"))
                .await
                .context(SetChannelDisplaySnafu)?;
        }
        OscilloscopeCommand::SetChannelVOffset(channel, offset) => {
            // HACK: 0.0001 because the rounding/float parsing on the device is a bit wonky
            let offset = offset + offset.signum() * 0.0001;
            io.send_with_writer(|w| write!(w, ":{channel}:OFFSet {offset:.4}"))
                .await
                .context(SetChannelVOffsetSnafu)?;
        }
        OscilloscopeCommand::SetChannelVScale(channel, scale) => {
            io.send_with_writer(|w| write!(w, ":{channel}:SCALe {scale:.2}"))
                .await
                .context(SetChannelVScaleSnafu { at: "send set" })?;
            // make sure the device is ready again
            io.send_with_writer(|w| write!(w, ":{channel}:SCALe?"))
                .await
                .context(SetChannelVScaleSnafu {
                    at: "send retrieve",
                })?;
            let _ = io.recv(buf).await.context(SetChannelVScaleSnafu {
                at: "recv retrieve",
            })?;
        }
        OscilloscopeCommand::SetChannelCoupling(channel, coupling) => {
            io.send_with_writer(|w| write!(w, ":{channel}:COUPling {coupling}"))
                .await
                .context(SetChannelCouplingSnafu)?;
        }
        OscilloscopeCommand::SetChannelAttenuation(channel, att) => {
            io.send_with_writer(|w| write!(w, ":{channel}:PROBe {att}"))
                .await
                .context(SetChannelAttenuationSnafu)?;
        }
        OscilloscopeCommand::SetTimeScale(time) => {
            io.send_with_writer(|w| write!(w, ":HORIzontal:SCALe {time:#}"))
                .await
                .context(SetTimeScaleSnafu { at: "send set" })?;
            // make sure the device is ready again
            let _ = io
                .send_with_output(b":HORIzontal:SCALe?", buf)
                .await
                .context(SetTimeScaleSnafu {
                    at: "send/recv retrieve",
                })?;
        }
        OscilloscopeCommand::SetTriggerSource(channel) => {
            io.send_with_writer(|w| write!(w, ":TRIGger:SINGle:SOURce {channel}"))
                .await
                .context(SetTriggerSourceSnafu)?;
        }
        OscilloscopeCommand::SetTriggerEdge(edge) => {
            io.send_with_writer(|w| write!(w, ":TRIGger:SINGle:EDGe {edge}"))
                .await
                .context(SetTriggerEdgeSnafu)?;
        }
        OscilloscopeCommand::SetTriggerLevel(voltage) => {
            // HACK: 0.0001 because the rounding/float parsing on the device is a bit wonky
            let voltage = Voltage(voltage.0 + voltage.0.signum() * 0.0001);
            io.send_with_writer(|w| write!(w, ":TRIGger:SINGle:EDGe:LEVel {voltage}"))
                .await
                .context(SetTriggerLevelSnafu)?;
        }
        OscilloscopeCommand::SetTriggerSweep(sweep) => {
            io.send_with_writer(|w| write!(w, ":TRIGger:SINGle:SWEep {sweep}"))
                .await
                .context(SetTriggerSweepSnafu)?;
        }
        OscilloscopeCommand::SetTriggerCoupling(coupling) => {
            io.send_with_writer(|w| write!(w, ":TRIGger:SINGle:COUPling {coupling}"))
                .await
                .context(SetTriggerCouplingSnafu)?;
        }
        OscilloscopeCommand::SetAcquisitionMode(ty) => {
            io.send_with_writer(|w| write!(w, ":ACQuire:MODe {ty}"))
                .await
                .context(SetAcquisitionModeSnafu)?;
        }
        OscilloscopeCommand::SetAcquisitionDepth(d) => {
            io.send_with_writer(|w| write!(w, ":ACQuire:DEPMem {d}"))
                .await
                .context(SetAcquisitionDepthSnafu)?;
        }
        OscilloscopeCommand::Auto => {
            io.send(b":AUToset .").await.context(AutoSnafu)?;
        }
    }

    Ok(())
}

async fn read_awg_config(io: &mut Io) -> Result<AwgConfig, ReadAwgConfigError> {
    let buf = &mut [0u8; 1024];

    Ok(AwgConfig {
        enabled: {
            let enabled = io.send_with_output(b":CHAN?", buf).await?;
            from_utf8(enabled)?
                .trim()
                .parse::<AwgChannelDisplay>()?
                .into()
        },
        mode: {
            let mode = io.send_with_output(b":FUNC?", buf).await?;
            from_utf8(mode)?.trim().parse()?
        },
        frequency: {
            let freq = io.send_with_output(b":FUNC:FREQ?", buf).await?;
            // BUG: frequency readout is micro-Hz for some reason
            Frequency(from_utf8(freq)?.trim().parse::<f64>()? / 1e6)
        },
        amplitude: {
            let volt = io.send_with_output(b":FUNC:AMPL?", buf).await?;
            // BUG: amplitude readout is millivolt for some reason
            Voltage(from_utf8(volt)?.trim().parse::<f64>()? / 1e3)
        },
        offset: {
            let volt = io.send_with_output(b":FUNC:OFFS?", buf).await?;
            // BUG: offset readout is millivolt for some reason
            Voltage(from_utf8(volt)?.trim().parse::<f64>()? / 1e3)
        },
    })
}

async fn set_awg_config(io: &mut Io, config: AwgConfig) -> Result<(), SetAwgConfigError> {
    io.send_with_writer(|w| write!(w, ":FUNC {}", config.mode))
        .await?;
    io.send_with_writer(|w| write!(w, ":FUNC:FREQ {}", config.frequency.0))
        .await?;
    io.send_with_writer(|w| write!(w, ":FUNC:AMPL {}", config.amplitude.0))
        .await?;
    io.send_with_writer(|w| write!(w, ":FUNC:OFFS {}", config.offset.0))
        .await?;
    io.send_with_writer(|w| write!(w, ":CHAN {}", AwgChannelDisplay::from(config.enabled)))
        .await?;

    Ok(())
}

async fn get_signal(
    io: &mut Io,
    ch0_enabled: bool,
    ch1_enabled: bool,
) -> Result<SignalData, AcquireSignalDataError> {
    let buf = &mut [0u8; 1024 + 4];
    let buf2 = &mut [0u8; 1024 + 4];
    let buf3 = &mut [0u8; 1024 + 4];

    let should_read_data = ch0_enabled || ch1_enabled;

    if should_read_data {
        if ch0_enabled {
            io.raw_send_nowait(b":DATa:WAVe:SCReen:CH1?")
                .await
                .context(SendSignalCmdSnafu {
                    channel: Some(Channel::Ch1),
                })?;
        } else {
            io.raw_send_nowait(b":DATa:WAVe:SCReen:CH2?")
                .await
                .context(SendSignalCmdSnafu {
                    channel: Some(Channel::Ch2),
                })?;
        }
    }
    io.raw_send_nowait(b":DATa:WAVe:SCReen:HEAD?")
        .await
        .context(SendSignalCmdSnafu { channel: None })?;

    let read1 = io
        .recv(buf)
        .await
        .context(RecvSignalSnafu { read_number: 1 })?;
    let (header, ch_data): (DataHeader, _) = if should_read_data {
        let read2 = io
            .recv(buf2)
            .await
            .context(RecvSignalSnafu { read_number: 2 })?;

        match serde_json::from_slice(&read2[4..]) {
            Ok(head) => (head, Some(read1)),
            Err(e) => (
                serde_json::from_slice(&read1[4..])
                    .context(DeserializeSignalHeaderSnafu { source2: Some(e) })?,
                Some(read2),
            ),
        }
    } else {
        (
            serde_json::from_slice(&read1[4..])
                .context(DeserializeSignalHeaderSnafu { source2: None })?,
            None,
        )
    };

    let ch_vec = ch_data.map(|d| {
        let mut ch_vec = ArrayVec::new();
        ch_vec.extend(d[4..].iter().copied());
        ch_vec
    });

    let ch_vec_2 = if should_read_data && ch1_enabled {
        io.raw_send_nowait(b":DATa:WAVe:SCReen:CH2?")
            .await
            .context(SendSignalCmdSnafu {
                channel: Some(Channel::Ch2),
            })?;
        let read3 = io
            .recv(buf3)
            .await
            .context(RecvSignalSnafu { read_number: 3 })?;

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
        header,
        ch0_data: ch_vec,
        ch1_data: ch_vec_2,
    })
}

async fn get_measurements(
    io: &mut Io,
    ch: Channel,
) -> Result<Measurements, AcquireMeasurementError> {
    let commands = Measurements::channel_to_measurement_commands(ch);

    let mut measurements = Measurements::default();
    let buf = &mut [0u8; 64];
    for cmd in commands {
        io.raw_send_nowait(cmd).await?;
        let read = io.recv(buf).await?;
        measurements.with_parsed(std::str::from_utf8(read)?);
    }

    Ok(measurements)
}

#[derive(Debug, Snafu)]
pub enum RunError {
    IoOpen {
        source: WindowsError,
    },
    #[snafu(transparent)]
    SendCommand {
        source: CommandIoError,
    },
    #[snafu(display("AcquireMeasurement({channel})"))]
    AcquireMeasurement {
        source: AcquireMeasurementError,
        channel: Channel,
    },
    #[snafu(transparent)]
    AcquireSignalData {
        source: AcquireSignalDataError,
    },
    #[snafu(transparent)]
    ReadAwgConfig {
        source: ReadAwgConfigError,
    },
    #[snafu(transparent)]
    SetAwgConfig {
        source: SetAwgConfigError,
    },
}

#[derive(Debug, Snafu)]
pub enum CommandIoError {
    SetHorizontalOffset {
        source: IoError,
    },
    SetChannelDisplay {
        source: IoError,
    },
    SetChannelVOffset {
        source: IoError,
    },
    #[snafu(display("SetChannelVScale(at: {at})"))]
    SetChannelVScale {
        source: IoError,
        at: &'static str,
    },
    SetChannelCoupling {
        source: IoError,
    },
    SetChannelAttenuation {
        source: IoError,
    },
    #[snafu(display("SetTimeScale(at: {at})"))]
    SetTimeScale {
        source: IoError,
        at: &'static str,
    },
    SetTriggerSource {
        source: IoError,
    },
    SetTriggerEdge {
        source: IoError,
    },
    SetTriggerLevel {
        source: IoError,
    },
    SetTriggerSweep {
        source: IoError,
    },
    SetTriggerCoupling {
        source: IoError,
    },
    SetAcquisitionMode {
        source: IoError,
    },
    SetAcquisitionDepth {
        source: IoError,
    },
    Auto {
        source: IoError,
    },
}

#[derive(Debug, Snafu)]
pub enum AcquireMeasurementError {
    #[snafu(context(false))]
    Io { source: IoError },
    #[snafu(context(false))]
    Utf8 { source: Utf8Error },
}

#[derive(Debug, Snafu)]
pub enum AcquireSignalDataError {
    #[snafu(display("SendSignalCmd({})", channel.map(|c| c.to_string()).unwrap_or_else(|| "header".to_string())))]
    SendSignalCmd {
        source: IoError,
        /// none if signal header
        channel: Option<Channel>,
    },
    #[snafu(display("RecvSignal({read_number})"))]
    RecvSignal { source: IoError, read_number: u8 },
    #[snafu(display("DeserializeSignalHeader(source2: {source2:?})"))]
    DeserializeSignalHeader {
        source: serde_json::Error,
        source2: Option<serde_json::Error>,
    },
}

#[derive(Debug, Snafu)]
pub enum ReadAwgConfigError {
    #[snafu(context(false))]
    Io { source: IoError },
    #[snafu(context(false))]
    Utf8 { source: Utf8Error },
    #[snafu(context(false))]
    Float { source: std::num::ParseFloatError },
    #[snafu(context(false))]
    Strum { source: strum::ParseError },
}

#[derive(Debug, Snafu)]
pub enum SetAwgConfigError {
    #[snafu(context(false))]
    Io { source: IoError },
}
