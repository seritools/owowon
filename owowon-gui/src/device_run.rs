use egui::Context;
use owowon::{
    device::{run_device_loop, Device, RunError},
    InitialDeviceRunConfig, OscilloscopeMessage, OscilloscopeRunCommand,
};
use std::thread;
use tokio::{sync::mpsc, task::LocalSet};

#[derive(Debug, Default)]
pub enum DeviceRunState {
    #[default]
    Stopped,
    Running(DeviceRun),
    Error(RunError),
}

impl DeviceRunState {
    pub fn is_running(&self) -> bool {
        matches!(self, DeviceRunState::Running(_))
    }

    pub fn update(&mut self) {
        match self {
            DeviceRunState::Stopped => {}
            DeviceRunState::Running(run) => {
                if !run.is_active() {
                    match run.join() {
                        None | Some(Ok(_)) => *self = DeviceRunState::Stopped,
                        Some(Err(e)) => *self = DeviceRunState::Error(e),
                    }
                }
            }
            DeviceRunState::Error(_) => {}
        }
    }

    pub fn stop(&mut self) {
        match self {
            DeviceRunState::Stopped => {}
            DeviceRunState::Running(run) => {
                let thread = run.data_thread.take().unwrap();
                // dropping the run, including the channels, causing the other thread to exit
                // eventually
                *self = DeviceRunState::Stopped;
                thread.join().unwrap().unwrap();
            }
            DeviceRunState::Error(_) => {}
        }
    }
}

#[derive(Debug)]
pub struct DeviceRun {
    data_thread: Option<thread::JoinHandle<Result<(), RunError>>>,
    message_rx: mpsc::Receiver<OscilloscopeMessage>,
    command_tx: mpsc::Sender<OscilloscopeRunCommand>,
}

impl DeviceRun {
    pub fn new(device: Device, egui_ctx: &Context, initial_config: InitialDeviceRunConfig) -> Self {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap();

        let (message_tx, message_rx) = mpsc::channel(32);
        let (command_tx, command_rx) = mpsc::channel(32);

        let egui_ctx = egui_ctx.clone();
        let data_thread = thread::Builder::new()
            .name("device-io".into())
            .spawn(move || {
                let local = LocalSet::new();

                let run = local.run_until(async move {
                    run_device_loop(device, message_tx, command_rx, initial_config, move || {
                        egui_ctx.request_repaint()
                    })
                    .await
                });

                rt.block_on(run)
            })
            .unwrap();

        let _ = command_tx.blocking_send(OscilloscopeRunCommand::RunSetting(
            owowon::OscilloscopeRunSetting::ReadAwgConfig,
        ));

        Self {
            data_thread: Some(data_thread),
            message_rx,
            command_tx,
        }
    }

    pub fn is_active(&self) -> bool {
        !self
            .data_thread
            .as_ref()
            .map(|t| t.is_finished())
            .unwrap_or(true)
    }

    pub fn join(&mut self) -> Option<Result<(), RunError>> {
        self.data_thread.take().map(|t| t.join().unwrap())
    }

    pub fn command_channel(&self) -> &mpsc::Sender<OscilloscopeRunCommand> {
        &self.command_tx
    }

    pub fn message_channel(&mut self) -> &mut mpsc::Receiver<OscilloscopeMessage> {
        &mut self.message_rx
    }
}
