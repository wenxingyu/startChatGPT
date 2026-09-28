//! Refresh cached quota state and reconnect without blocking the UI state lock.
use super::{
    bridge::Bridge,
    model::{State, parse},
};
use crate::config::ProxySetting;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

pub(crate) enum Action {
    Refresh,
    Stop,
}

pub(crate) type Notify = Box<dyn Fn() + Send>;

pub(crate) fn start_with_notify(
    app: PathBuf,
    proxy: ProxySetting,
    state: Arc<Mutex<State>>,
    notify: Option<Notify>,
) -> (mpsc::Sender<Action>, thread::JoinHandle<()>) {
    start_with_connector(move || Bridge::connect(&app, &proxy), state, notify)
}

fn start_with_connector(
    mut connect: impl FnMut() -> Result<Bridge, String> + Send + 'static,
    state: Arc<Mutex<State>>,
    notify: Option<Notify>,
) -> (mpsc::Sender<Action>, thread::JoinHandle<()>) {
    let (tx, rx) = mpsc::channel();
    let worker = thread::spawn(move || {
        let mut bridge = None;
        loop {
            let result = (|| {
                if bridge.is_none() {
                    bridge = Some(connect()?);
                }
                let result = bridge
                    .as_mut()
                    .unwrap()
                    .request("account/rateLimits/read", None)?;
                parse(&result)
            })();
            let failed = result.is_err();
            {
                let mut state = state.lock().unwrap();
                match result {
                    Ok(windows) => {
                        state.windows = windows;
                        state.updated = Some(Instant::now());
                        state.error = None;
                    }
                    Err(error) => {
                        state.error = Some(error);
                    }
                }
            }
            if let Some(notify) = &notify {
                notify();
            }
            // Process cleanup can block. Publish the error and release the UI's
            // state lock first, so details and the tray menu stay responsive.
            if failed {
                bridge = None;
            }
            match rx.recv_timeout(Duration::from_secs(45)) {
                Ok(Action::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                _ => {}
            }
            // Collapse repeated refresh clicks and honor shutdown before another request.
            if rx.try_iter().any(|a| matches!(a, Action::Stop)) {
                break;
            }
        }
    });
    (tx, worker)
}

#[cfg(test)]
mod recovery_tests;
