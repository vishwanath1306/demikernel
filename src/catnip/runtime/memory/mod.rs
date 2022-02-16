// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

mod config;
pub mod consts;
mod dpdkbuf;
mod manager;
mod mbuf;

//==============================================================================
// Exports
//==============================================================================

pub use dpdkbuf::DPDKBuf;
pub use manager::MemoryManager;

//==============================================================================
// Imports
//==============================================================================

use super::DPDKRuntime;
use ::runtime::{
    memory::MemoryRuntime,
    types::dmtr_sgarray_t,
};

//==============================================================================
// Trait Implementations
//==============================================================================

/// Memory Runtime Trait Implementation for DPDK Runtime
impl MemoryRuntime for DPDKRuntime {
    type Buf = DPDKBuf;

    /// Casts a [DPDKBuf] into an [dmtr_sgarray_t].
    fn into_sgarray(&self, buf: Self::Buf) -> dmtr_sgarray_t {
        self.memory_manager.into_sgarray(buf)
    }

    /// Allocates a [dmtr_sgarray_t].
    fn alloc_sgarray(&self, size: usize) -> dmtr_sgarray_t {
        self.memory_manager.alloc_sgarray(size)
    }

    /// Releases a [dmtr_sgarray_t].
    fn free_sgarray(&self, sga: dmtr_sgarray_t) {
        self.memory_manager.free_sgarray(sga)
    }

    /// Clones a [dmtr_sgarray_t].
    fn clone_sgarray(&self, sga: &dmtr_sgarray_t) -> Self::Buf {
        self.memory_manager.clone_sgarray(sga)
    }
}
