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

impl Default for RuntimeTopology {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::RuntimeTopology;
    use crate::controller::events::{ControllerEvent, ControllerOutput};
    use crate::controller::state::ControllerState;

    #[test]
    fn channels_round_trip_messages() {
        let topology = RuntimeTopology::new();

        topology
            .controller_event_tx
            .send(ControllerEvent::Toggle)
            .expect("send event");
        let event = topology.controller_event_rx.recv().expect("recv event");
        assert!(matches!(event, ControllerEvent::Toggle));

        topology
            .controller_output_tx
            .send(ControllerOutput::StateChanged(ControllerState::Idle))
            .expect("send output");
        let output = topology.controller_output_rx.recv().expect("recv output");
        assert!(matches!(
            output,
            ControllerOutput::StateChanged(ControllerState::Idle)
        ));
    }
}
