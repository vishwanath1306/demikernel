// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.
use crate::protocols::tcp::constants::{
    DEFAULT_MSS,
    MAX_MSS,
    MIN_MSS,
};
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct TcpOptions {
    pub advertised_mss: usize,
    pub handshake_retries: usize,
    pub handshake_timeout: Duration,
    pub receive_window_size: usize,
    pub retries: usize,
    pub trailing_ack_delay: Duration,
}

impl Default for TcpOptions {
    fn default() -> Self {
        let receive_window_size = std::env::var("MAX_WINDOW_SIZE").unwrap().parse().unwrap();
        let mss: usize = std::env::var("MSS").unwrap().parse().unwrap();
        TcpOptions {
            advertised_mss: mss,
            handshake_retries: 5,
            handshake_timeout: Duration::from_secs(3),
            receive_window_size,
            retries: 5,
            trailing_ack_delay: Duration::from_micros(1),
        }
    }
}

impl TcpOptions {
    pub fn advertised_mss(mut self, value: usize) -> Self {
        assert!(value >= MIN_MSS);
        assert!(value <= MAX_MSS);
        self.advertised_mss = value;
        self
    }

    pub fn handshake_retries(mut self, value: usize) -> Self {
        assert!(value > 0);
        self.handshake_retries = value;
        self
    }

    pub fn handshake_timeout(mut self, value: Duration) -> Self {
        assert!(value > Duration::new(0, 0));
        self.handshake_timeout = value;
        self
    }

    pub fn receive_window_size(mut self, value: usize) -> Self {
        assert!(value > 0);
        self.receive_window_size = value;
        self
    }

    pub fn retries(mut self, value: usize) -> Self {
        assert!(value > 0);
        self.retries = value;
        self
    }

    pub fn trailing_ack_delay(mut self, value: Duration) -> Self {
        self.trailing_ack_delay = value;
        self
    }
}
