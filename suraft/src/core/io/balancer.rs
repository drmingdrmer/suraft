//! This mod defines and manipulates the ratio of multiple messages to handle.
//!
//! The ratio should be adjust so that every channel won't be starved.

/// Balance the ratio of different kind of message to handle.
pub(crate) struct Balancer {
    total: u64,

    /// The number of APIMessage to handle in each round.
    api_message: u64,
}

impl Balancer {
    pub(crate) fn new(total: u64) -> Self {
        Self {
            total,
            // APIMessage is the input entry.
            // We should consume as many as internal messages as possible.
            api_message: total / 10,
        }
    }

    pub(crate) fn api_message(&self) -> u64 {
        self.api_message
    }

    pub(crate) fn notification(&self) -> u64 {
        self.total - self.api_message
    }

    pub(crate) fn increase_notification(&mut self) {
        self.api_message = self.api_message * 15 / 16;
        if self.api_message == 0 {
            self.api_message = 1;
        }
    }

    pub(crate) fn increase_api_message(&mut self) {
        self.api_message = self.api_message * 17 / 16;

        // Always leave some budget for other channels
        if self.api_message > self.total / 2 {
            self.api_message = self.total / 2;
        }
    }
}
