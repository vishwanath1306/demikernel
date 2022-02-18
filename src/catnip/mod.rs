// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

pub mod bindings;
mod dpdk;
mod runtime;

//==============================================================================
// Imports
//==============================================================================

use self::runtime::DPDKRuntime;
use ::catnip::libos::LibOS;
use std::ops::{
    Deref,
    DerefMut,
};

//==============================================================================
// Exports
//==============================================================================

pub use self::{
    bindings::catnip_init,
    runtime::memory::DPDKBuf,
};

//==============================================================================
// Structures
//==============================================================================

/// Catnip LibOS
pub struct CatnipLibos(LibOS<DPDKRuntime>);

//==============================================================================
// Trait Implementations
//==============================================================================

/// De-Reference Trait Implementation for Catnip LibOS
impl Deref for CatnipLibos {
    type Target = LibOS<DPDKRuntime>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Mutable De-Reference Trait Implementation for Catnip LibOS
impl DerefMut for CatnipLibos {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
