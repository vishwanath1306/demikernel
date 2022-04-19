// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use super::config::TestConfig;
use demikernel::{
    Ipv4Endpoint,
    LibOS,
};
use runtime::memory::{
    Buffer,
    Bytes,
};

//==============================================================================
// Test
//==============================================================================

pub struct Test {
    config: TestConfig,
    pub libos: LibOS,
}

impl Test {
    pub fn new() -> Self {
        let config: TestConfig = TestConfig::new();
        let libos: LibOS = LibOS::new();

        Self { config, libos }
    }

    pub fn is_server(&self) -> bool {
        self.config.is_server()
    }

    pub fn local_addr(&self) -> Ipv4Endpoint {
        self.config.local_addr()
    }

    pub fn remote_addr(&self) -> Ipv4Endpoint {
        self.config.remote_addr()
    }

    pub fn get_buffer_size(&self) -> usize {
        self.config.0.get_buffer_size()
    }

    pub fn mkbuf(&self, fill_char: u8) -> Vec<u8> {
        assert!(self.get_buffer_size() <= self.config.0.mss);

        let mut data: Vec<u8> = Vec::<u8>::with_capacity(self.get_buffer_size());

        for _ in 0..self.get_buffer_size() {
            data.push(fill_char);
        }

        data
    }

    pub fn bufcmp(x: &[u8], b: Bytes) -> bool {
        let a: Bytes = Bytes::from_slice(x);
        if a.len() != b.len() {
            return false;
        }

        for i in 0..a.len() {
            if a[i] != b[i] {
                return false;
            }
        }

        true
    }
}
