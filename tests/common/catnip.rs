// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use super::config::TestConfig;
use ::catnip::{
    libos::LibOS,
    protocols::ipv4::Ipv4Endpoint,
};
use ::dpdk_rs::load_mlx_driver;
use demikernel::catnip::{
    dpdk::initialize_dpdk,
    runtime::{
        memory::DPDKBuf,
        DPDKRuntime,
    },
};
use runtime::memory::Buffer;

//==============================================================================
// Test
//==============================================================================

pub struct Test {
    config: TestConfig,
    pub libos: LibOS<DPDKRuntime>,
}

impl Test {
    pub fn new() -> Self {
        load_mlx_driver();
        let config: TestConfig = TestConfig::new();
        let rt = initialize_dpdk(
            config.0.local_ipv4_addr,
            &config.0.eal_init_args(),
            config.0.arp_table(),
            config.0.disable_arp,
            config.0.use_jumbo_frames,
            config.0.mtu,
            config.0.mss,
            config.0.tcp_checksum_offload,
            config.0.udp_checksum_offload,
        )
        .unwrap();
        let libos = LibOS::new(rt).unwrap();

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
