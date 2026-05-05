use std::io::{self, IsTerminal};
use std::sync::{Arc, atomic::AtomicBool};

use orca_core::output::{OutputHandle, OutputObserver};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

pub mod plain;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Auto,
    Plain,
    Json,
    Tui,
}

pub struct OutputTask {
    handle: Option<OutputHandle>,
    join: Option<JoinHandle<()>>,
}

impl OutputTask {
    pub fn start(mode: OutputMode) -> Self {
        Self::start_with_observer(mode, None)
    }

    pub fn start_with_observer(mode: OutputMode, observer: Option<OutputObserver>) -> Self {
        if mode == OutputMode::Json {
            return Self {
                handle: None,
                join: None,
            };
        }

        let selected = select_mode(mode);
        let (sender, receiver) = mpsc::unbounded_channel();
        let stop_requested = Arc::new(AtomicBool::new(false));
        let handle = OutputHandle::new(sender, Arc::clone(&stop_requested), observer);
        let join = match selected {
            SelectedOutputMode::Plain => tokio::spawn(plain::run(receiver)),
            SelectedOutputMode::Tui => tokio::spawn(crate::tui::run(receiver, stop_requested)),
        };

        Self {
            handle: Some(handle),
            join: Some(join),
        }
    }

    pub fn handle(&self) -> Option<OutputHandle> {
        self.handle.clone()
    }

    pub async fn shutdown(mut self) {
        if let Some(handle) = &self.handle {
            handle.shutdown();
        }
        if let Some(join) = self.join.take() {
            let _ = join.await;
        }
    }
}

impl Drop for OutputTask {
    fn drop(&mut self) {
        if let Some(handle) = &self.handle {
            handle.shutdown();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectedOutputMode {
    Plain,
    Tui,
}

fn select_mode(mode: OutputMode) -> SelectedOutputMode {
    match mode {
        OutputMode::Tui if io::stdout().is_terminal() => SelectedOutputMode::Tui,
        OutputMode::Auto if io::stdout().is_terminal() => SelectedOutputMode::Tui,
        OutputMode::Auto | OutputMode::Plain | OutputMode::Tui | OutputMode::Json => {
            SelectedOutputMode::Plain
        }
    }
}
