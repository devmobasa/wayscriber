//! Capacity-one verified clipboard publication worker.

use std::sync::{Arc, mpsc};
use std::thread;

use crate::pin::PinId;
use crate::pin::PinMemoryCharge;

#[derive(Debug)]
pub(crate) struct ClipboardCompletion {
    pub(crate) pin_id: PinId,
    pub(crate) generation: u64,
    pub(crate) result: Result<(), String>,
    pub(crate) retained_charge: Option<PinMemoryCharge>,
}

#[derive(Default)]
pub(crate) struct ClipboardWorker {
    active: Option<ActiveCopy>,
}

struct ActiveCopy {
    pin_id: PinId,
    generation: u64,
    receiver: mpsc::Receiver<ClipboardCompletion>,
    worker: thread::JoinHandle<()>,
    retained_charge: Option<PinMemoryCharge>,
}

impl ClipboardWorker {
    /// Returns false when an existing publication absorbed this request.
    pub(crate) fn start(
        &mut self,
        pin_id: PinId,
        generation: u64,
        png: Arc<Vec<u8>>,
    ) -> Result<bool, String> {
        if self.active.is_some() {
            return Ok(false);
        }
        let (sender, receiver) = mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name("wayscriber-pin-clipboard".to_string())
            .spawn(move || {
                let result =
                    std::panic::catch_unwind(|| crate::capture::clipboard::copy_to_clipboard(&png))
                        .map_err(|_| "clipboard worker panicked".to_string())
                        .and_then(|result| result.map_err(|error| error.to_string()));
                let _ = sender.send(ClipboardCompletion {
                    pin_id,
                    generation,
                    result,
                    retained_charge: None,
                });
            })
            .map_err(|error| format!("failed to start clipboard worker: {error}"))?;
        self.active = Some(ActiveCopy {
            pin_id,
            generation,
            receiver,
            worker,
            retained_charge: None,
        });
        Ok(true)
    }

    pub(crate) fn poll(&mut self) -> Option<ClipboardCompletion> {
        let completion = match self.active.as_ref()?.receiver.try_recv() {
            Ok(completion) => completion,
            Err(mpsc::TryRecvError::Empty) => return None,
            Err(mpsc::TryRecvError::Disconnected) => ClipboardCompletion {
                // The sender always owns these values until it sends, so a
                // disconnected channel means a panic escaped before recovery.
                // Retain no fake pin identity; join below and let the host log.
                pin_id: self.active.as_ref()?.pin_id,
                generation: self.active.as_ref()?.generation,
                result: Err("clipboard worker disconnected".to_string()),
                retained_charge: None,
            },
        };
        if let Some(active) = self.active.take() {
            let mut completion = completion;
            completion.retained_charge = active.retained_charge;
            let _ = active.worker.join();
            return Some(completion);
        }
        None
    }

    pub(crate) fn is_active(&self) -> bool {
        self.active.is_some()
    }

    pub(crate) fn active_pin(&self) -> Option<PinId> {
        self.active.as_ref().map(|active| active.pin_id)
    }

    pub(crate) fn retain_charge(&mut self, pin_id: PinId, charge: PinMemoryCharge) -> bool {
        let Some(active) = self
            .active
            .as_mut()
            .filter(|active| active.pin_id == pin_id)
        else {
            return false;
        };
        active.retained_charge = Some(charge);
        true
    }
}

impl Drop for ClipboardWorker {
    fn drop(&mut self) {
        if let Some(active) = self.active.take() {
            let _ = active.worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capacity_one_absorbs_a_second_copy() {
        let (sender, receiver) = mpsc::channel();
        let worker = thread::spawn(move || {
            let _ = sender.send(());
        });
        let mut controller = ClipboardWorker {
            active: Some(ActiveCopy {
                pin_id: PinId::new(1).unwrap(),
                generation: 1,
                receiver: {
                    let (_completion_sender, completion_receiver) = mpsc::channel();
                    completion_receiver
                },
                worker,
                retained_charge: None,
            }),
        };
        assert!(
            !controller
                .start(PinId::new(1).unwrap(), 1, Arc::new(b"png".to_vec()))
                .unwrap()
        );
        let _ = receiver.recv();
    }
}
