// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use super::config::TestConfig;
use ::catnip::protocols::ipv4::Ipv4Endpoint;
use demikernel::catnip::{
    catnip_init,
    CatnipLibos,
    DPDKBuf,
};
use runtime::memory::Buffer;

//==============================================================================
// Test
//==============================================================================

pub struct Test {
    config: TestConfig,
    pub libos: CatnipLibos,
}

impl Test {
    pub fn new() -> Self {
        let config: TestConfig = TestConfig::new();
        let libos = catnip_init(None, None).unwrap();

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

    pub fn mkbuf(&self, fill_char: u8) -> DPDKBuf {
        let a: Vec<u8> = (0..self.config.0.buffer_size).map(|_| fill_char).collect();
        DPDKBuf::from_slice(&a)
    }

    pub fn bufcmp(a: DPDKBuf, b: DPDKBuf) -> bool {
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
