//! Independently stored input FIFOs.
use crate::{AgentMessage, Queue, QueueMode};
use std::collections::VecDeque;
#[derive(Default)]
pub(crate) struct Queues {
    steering: VecDeque<AgentMessage>,
    follow_up: VecDeque<AgentMessage>,
    pub steering_mode: QueueMode,
    pub follow_up_mode: QueueMode,
}
impl Queues {
    pub fn mode(&mut self, queue: Queue) -> &mut QueueMode {
        match queue {
            Queue::Steering => &mut self.steering_mode,
            Queue::FollowUp => &mut self.follow_up_mode,
        }
    }
    pub fn drain(&mut self, queue: Queue) -> Vec<AgentMessage> {
        let count = match *self.mode(queue) {
            QueueMode::OneAtATime => 1,
            QueueMode::All => self.get(queue).len(),
        };
        let fifo = self.get(queue);
        fifo.drain(..count.min(fifo.len())).collect()
    }
    pub fn get(&mut self, queue: Queue) -> &mut VecDeque<AgentMessage> {
        match queue {
            Queue::Steering => &mut self.steering,
            Queue::FollowUp => &mut self.follow_up,
        }
    }
}
