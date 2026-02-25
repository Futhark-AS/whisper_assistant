use crossbeam_channel::{Receiver, Sender};

use crate::controller::events::{ControllerEvent, ControllerOutput};

pub struct RuntimeTopology {
    pub controller_event_tx: Sender<ControllerEvent>,
    pub controller_event_rx: Receiver<ControllerEvent>,
    pub controller_output_tx: Sender<ControllerOutput>,
    pub controller_output_rx: Receiver<ControllerOutput>,
}

impl RuntimeTopology {
    pub fn new() -> Self {
        let (controller_event_tx, controller_event_rx) = crossbeam_channel::unbounded();
        let (controller_output_tx, controller_output_rx) = crossbeam_channel::unbounded();

        Self {
            controller_event_tx,
            controller_event_rx,
            controller_output_tx,
            controller_output_rx,
        }
    }
}
