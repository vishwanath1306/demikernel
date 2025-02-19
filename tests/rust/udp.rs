// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

mod common;

//==============================================================================
// Imports
//==============================================================================

use ::anyhow::Result;
use ::demikernel::{
    inetstack::InetStack,
    runtime::{
        memory::DemiBuffer,
        OperationResult,
        QDesc,
        QToken,
    },
};
use common::{
    arp,
    libos::*,
    ALICE_IPV4,
    ALICE_MAC,
    BOB_IPV4,
    BOB_MAC,
    PORT_BASE,
};
use crossbeam_channel::{
    self,
    Receiver,
    Sender,
};
use demikernel::runtime::network::consts::RECEIVE_BATCH_SIZE;

#[cfg(target_os = "windows")]
pub const AF_INET: i32 = windows::Win32::Networking::WinSock::AF_INET.0 as i32;

#[cfg(target_os = "windows")]
pub const SOCK_DGRAM: i32 = windows::Win32::Networking::WinSock::SOCK_DGRAM as i32;

#[cfg(target_os = "linux")]
pub const AF_INET: i32 = libc::AF_INET;

#[cfg(target_os = "linux")]
pub const SOCK_DGRAM: i32 = libc::SOCK_DGRAM;

use std::{
    net::SocketAddrV4,
    thread::{
        self,
        JoinHandle,
    },
};

//==============================================================================
// Connect
//==============================================================================

/// Opens and closes a socket using a non-ephemeral port.
fn do_udp_setup<const N: usize>(libos: &mut InetStack<N>) -> Result<()> {
    let local: SocketAddrV4 = SocketAddrV4::new(ALICE_IPV4, PORT_BASE);
    let sockfd: QDesc = match libos.socket(AF_INET, SOCK_DGRAM, 0) {
        Ok(qd) => qd,
        Err(e) => anyhow::bail!("failed to create socket: {:?}", e),
    };
    match libos.bind(sockfd, local) {
        Ok(_) => (),
        Err(e) => {
            // Close socket on error.
            // FIXME: https://github.com/demikernel/demikernel/issues/633
            anyhow::bail!("bind() failed: {:?}", e)
        },
    };

    match libos.close(sockfd) {
        Ok(_) => Ok(()),
        Err(e) => anyhow::bail!("close() failed: {:?}", e),
    }
}

/// Opens and closes a socket using an ephemeral port.
fn do_udp_setup_ephemeral<const N: usize>(libos: &mut InetStack<N>) -> Result<()> {
    const PORT_EPHEMERAL_BASE: u16 = 49152;
    let local: SocketAddrV4 = SocketAddrV4::new(ALICE_IPV4, PORT_EPHEMERAL_BASE);
    let sockfd: QDesc = match libos.socket(AF_INET, SOCK_DGRAM, 0) {
        Ok(qd) => qd,
        Err(e) => anyhow::bail!("failed to create socket: {:?}", e),
    };
    match libos.bind(sockfd, local) {
        Ok(_) => (),
        Err(e) => {
            // Close socket on error.
            // FIXME: https://github.com/demikernel/demikernel/issues/633
            anyhow::bail!("bind() failed: {:?}", e)
        },
    };

    match libos.close(sockfd) {
        Ok(_) => Ok(()),
        Err(e) => anyhow::bail!("close() failed: {:?}", e),
    }
}

/// Opens and closes a socket using wildcard ephemeral port.
fn do_udp_setup_wildcard_ephemeral<const N: usize>(libos: &mut InetStack<N>) -> Result<()> {
    let local: SocketAddrV4 = SocketAddrV4::new(ALICE_IPV4, 0);
    let sockfd: QDesc = match libos.socket(AF_INET, SOCK_DGRAM, 0) {
        Ok(qd) => qd,
        Err(e) => anyhow::bail!("failed to create socket: {:?}", e),
    };
    match libos.bind(sockfd, local) {
        Ok(_) => (),
        Err(e) => {
            // Close socket on error.
            // FIXME: https://github.com/demikernel/demikernel/issues/633
            anyhow::bail!("bind() failed: {:?}", e)
        },
    };

    match libos.close(sockfd) {
        Ok(_) => Ok(()),
        Err(e) => anyhow::bail!("close() failed: {:?}", e),
    }
}

/// Tests if a socket can be successfully setup.
#[test]
fn udp_setup() -> Result<()> {
    let (tx, rx): (Sender<DemiBuffer>, Receiver<DemiBuffer>) = crossbeam_channel::unbounded();
    let mut libos: InetStack<RECEIVE_BATCH_SIZE> = match DummyLibOS::new(ALICE_MAC, ALICE_IPV4, tx, rx, arp()) {
        Ok(libos) => libos,
        Err(e) => anyhow::bail!("Could not create inetstack: {:?}", e),
    };

    do_udp_setup(&mut libos)?;
    do_udp_setup_ephemeral(&mut libos)?;
    do_udp_setup_wildcard_ephemeral(&mut libos)?;

    Ok(())
}

/// Tests if a connection can be successfully established in loopback mode.
#[test]
fn udp_connect_loopback() -> Result<()> {
    let (tx, rx): (Sender<DemiBuffer>, Receiver<DemiBuffer>) = crossbeam_channel::unbounded();
    let mut libos: InetStack<RECEIVE_BATCH_SIZE> = match DummyLibOS::new(ALICE_MAC, ALICE_IPV4, tx, rx, arp()) {
        Ok(libos) => libos,
        Err(e) => anyhow::bail!("Could not create inetstack: {:?}", e),
    };

    let port: u16 = PORT_BASE;
    let local: SocketAddrV4 = SocketAddrV4::new(ALICE_IPV4, port);

    // Open and close a connection.
    let sockfd: QDesc = match libos.socket(AF_INET, SOCK_DGRAM, 0) {
        Ok(qd) => qd,
        Err(e) => anyhow::bail!("failed to create socket: {:?}", e),
    };
    match libos.bind(sockfd, local) {
        Ok(_) => (),
        Err(e) => {
            // Close socket on error.
            // FIXME: https://github.com/demikernel/demikernel/issues/633
            anyhow::bail!("bind() failed: {:?}", e)
        },
    };

    match libos.close(sockfd) {
        Ok(_) => Ok(()),
        Err(e) => anyhow::bail!("close() failed: {:?}", e),
    }
}

//==============================================================================
// Push
//==============================================================================

/// Tests if data can be successfully pushed/popped form a local endpoint to
/// itself.
#[test]
fn udp_push_remote() -> Result<()> {
    let (alice_tx, alice_rx): (Sender<DemiBuffer>, Receiver<DemiBuffer>) = crossbeam_channel::unbounded();
    let (bob_tx, bob_rx): (Sender<DemiBuffer>, Receiver<DemiBuffer>) = crossbeam_channel::unbounded();

    let bob_port: u16 = PORT_BASE;
    let bob_addr: SocketAddrV4 = SocketAddrV4::new(BOB_IPV4, bob_port);
    let alice_port: u16 = PORT_BASE;
    let alice_addr: SocketAddrV4 = SocketAddrV4::new(ALICE_IPV4, alice_port);

    let alice: JoinHandle<Result<()>> = thread::spawn(move || {
        let mut libos: InetStack<RECEIVE_BATCH_SIZE> =
            match DummyLibOS::new(ALICE_MAC, ALICE_IPV4, alice_tx, bob_rx, arp()) {
                Ok(libos) => libos,
                Err(e) => anyhow::bail!("Could not create inetstack: {:?}", e),
            };

        // Open connection.
        let sockfd: QDesc = match libos.socket(AF_INET, SOCK_DGRAM, 0) {
            Ok(qd) => qd,
            Err(e) => anyhow::bail!("failed to create socket: {:?}", e),
        };
        match libos.bind(sockfd, alice_addr) {
            Ok(_) => (),
            Err(e) => {
                // Close socket on error.
                // FIXME: https://github.com/demikernel/demikernel/issues/633
                anyhow::bail!("bind() failed: {:?}", e)
            },
        }

        // Cook some data.
        let bytes: DemiBuffer = DummyLibOS::cook_data(32);

        // Push data.
        let qt: QToken = match libos.pushto2(sockfd, &bytes, bob_addr) {
            Ok(qt) => qt,
            Err(e) => {
                // Close socket on error.
                // FIXME: https://github.com/demikernel/demikernel/issues/633
                anyhow::bail!("push() failed: {:?}", e)
            },
        };
        let (_, qr): (QDesc, OperationResult) = safe_wait2(&mut libos, qt)?;
        match qr {
            OperationResult::Push => (),
            _ => {
                // Close socket on error.
                // FIXME: https://github.com/demikernel/demikernel/issues/633
                anyhow::bail!("wait on push() failed")
            },
        }

        // Pop data.
        let qt: QToken = match libos.pop(sockfd, None) {
            Ok(qt) => qt,
            Err(e) => {
                // Close socket on error.
                // FIXME: https://github.com/demikernel/demikernel/issues/633
                anyhow::bail!("pop()) failed: {:?}", e)
            },
        };
        let (_, qr): (QDesc, OperationResult) = safe_wait2(&mut libos, qt)?;
        match qr {
            OperationResult::Pop(_, _) => (),
            _ => {
                // Close socket on error.
                // FIXME: https://github.com/demikernel/demikernel/issues/633
                anyhow::bail!("wait on pop() failed")
            },
        }

        // Close connection.
        match libos.close(sockfd) {
            Ok(_) => Ok(()),
            Err(e) => anyhow::bail!("close() failed: {:?}", e),
        }
    });

    let bob: JoinHandle<Result<()>> = thread::spawn(move || {
        let mut libos: InetStack<RECEIVE_BATCH_SIZE> = match DummyLibOS::new(BOB_MAC, BOB_IPV4, bob_tx, alice_rx, arp())
        {
            Ok(libos) => libos,
            Err(e) => anyhow::bail!("Could not create inetstack: {:?}", e),
        };

        // Open connection.
        let sockfd: QDesc = match libos.socket(AF_INET, SOCK_DGRAM, 0) {
            Ok(qd) => qd,
            Err(e) => anyhow::bail!("failed to create socket: {:?}", e),
        };
        match libos.bind(sockfd, bob_addr) {
            Ok(_) => (),
            Err(e) => {
                // Close socket on error.
                // FIXME: https://github.com/demikernel/demikernel/issues/633
                anyhow::bail!("bind() failed: {:?}", e)
            },
        };

        // Pop data.
        let qt: QToken = match libos.pop(sockfd, None) {
            Ok(qt) => qt,
            Err(e) => {
                // Close socket on error.
                // FIXME: https://github.com/demikernel/demikernel/issues/633
                anyhow::bail!("pop() failed: {:?}", e)
            },
        };
        let (_, qr): (QDesc, OperationResult) = safe_wait2(&mut libos, qt)?;
        let bytes: DemiBuffer = match qr {
            OperationResult::Pop(_, bytes) => bytes,
            _ => {
                // Close socket on error.
                // FIXME: https://github.com/demikernel/demikernel/issues/633
                anyhow::bail!("wait on pop() failed")
            },
        };

        // Push data.
        let qt: QToken = match libos.pushto2(sockfd, &bytes, alice_addr) {
            Ok(qt) => qt,
            Err(e) => {
                // Close socket on error.
                // FIXME: https://github.com/demikernel/demikernel/issues/633
                anyhow::bail!("push() failed: {:?}", e)
            },
        };
        let (_, qr): (QDesc, OperationResult) = safe_wait2(&mut libos, qt)?;
        match qr {
            OperationResult::Push => (),
            _ => {
                // Close socket on error.
                // FIXME: https://github.com/demikernel/demikernel/issues/633
                anyhow::bail!("wait on push() failed")
            },
        }

        // Close connection.
        match libos.close(sockfd) {
            Ok(_) => Ok(()),
            Err(e) => anyhow::bail!("close() failed: {:?}", e),
        }
    });

    // It is safe to use unwrap here because there should not be any reason that we can't join the thread and if there
    // is, there is nothing to clean up here on the main thread.
    alice.join().unwrap()?;
    bob.join().unwrap()?;

    Ok(())
}

/// Tests if data can be successfully pushed/popped in loopback mode.
#[test]
fn udp_loopback() -> Result<()> {
    let (alice_tx, alice_rx): (Sender<DemiBuffer>, Receiver<DemiBuffer>) = crossbeam_channel::unbounded();
    let (bob_tx, bob_rx): (Sender<DemiBuffer>, Receiver<DemiBuffer>) = crossbeam_channel::unbounded();

    let bob_port: u16 = PORT_BASE;
    let bob_addr: SocketAddrV4 = SocketAddrV4::new(ALICE_IPV4, bob_port);
    let alice_port: u16 = PORT_BASE;
    let alice_addr: SocketAddrV4 = SocketAddrV4::new(ALICE_IPV4, alice_port);

    let alice: JoinHandle<Result<()>> = thread::spawn(move || {
        let mut libos: InetStack<RECEIVE_BATCH_SIZE> =
            match DummyLibOS::new(ALICE_MAC, ALICE_IPV4, alice_tx, bob_rx, arp()) {
                Ok(libos) => libos,
                Err(e) => anyhow::bail!("Could not create inetstack: {:?}", e),
            };

        // Open connection.
        let sockfd: QDesc = match libos.socket(AF_INET, SOCK_DGRAM, 0) {
            Ok(qd) => qd,
            Err(e) => anyhow::bail!("failed to create socket: {:?}", e),
        };
        match libos.bind(sockfd, alice_addr) {
            Ok(_) => (),
            Err(e) => {
                // Close socket on error.
                // FIXME: https://github.com/demikernel/demikernel/issues/633
                anyhow::bail!("bind() failed: {:?}", e)
            },
        };
        // Cook some data.
        let bytes: DemiBuffer = DummyLibOS::cook_data(32);

        // Push data.
        let qt: QToken = match libos.pushto2(sockfd, &bytes, bob_addr) {
            Ok(qt) => qt,
            Err(e) => {
                // Close socket on error.
                // FIXME: https://github.com/demikernel/demikernel/issues/633
                anyhow::bail!("push() failed: {:?}", e)
            },
        };
        let (_, qr): (QDesc, OperationResult) = safe_wait2(&mut libos, qt)?;
        match qr {
            OperationResult::Push => (),
            _ => {
                // Close socket on error.
                // FIXME: https://github.com/demikernel/demikernel/issues/633
                anyhow::bail!("wait on push() failed")
            },
        }

        // Pop data.
        let qt: QToken = match libos.pop(sockfd, None) {
            Ok(qt) => qt,
            Err(e) => {
                // Close socket on error.
                // FIXME: https://github.com/demikernel/demikernel/issues/633
                anyhow::bail!("pop() failed: {:?}", e)
            },
        };
        let (_, qr): (QDesc, OperationResult) = safe_wait2(&mut libos, qt)?;
        match qr {
            OperationResult::Pop(_, _) => (),
            _ => {
                // Close socket on error.
                // FIXME: https://github.com/demikernel/demikernel/issues/633
                anyhow::bail!("wait on pop() failed")
            },
        }

        // Close connection.
        match libos.close(sockfd) {
            Ok(_) => Ok(()),
            Err(e) => anyhow::bail!("close() failed: {:?}", e),
        }
    });

    let bob = thread::spawn(move || {
        let mut libos: InetStack<RECEIVE_BATCH_SIZE> =
            match DummyLibOS::new(ALICE_MAC, ALICE_IPV4, bob_tx, alice_rx, arp()) {
                Ok(libos) => libos,
                Err(e) => anyhow::bail!("Could not create inetstack: {:?}", e),
            };

        // Open connection.
        let sockfd: QDesc = match libos.socket(AF_INET, SOCK_DGRAM, 0) {
            Ok(qd) => qd,
            Err(e) => anyhow::bail!("failed to create socket: {:?}", e),
        };
        match libos.bind(sockfd, bob_addr) {
            Ok(_) => (),
            Err(e) => {
                // Close socket on error.
                // FIXME: https://github.com/demikernel/demikernel/issues/633
                anyhow::bail!("bind() failed: {:?}", e)
            },
        };
        // Pop data.
        let qt: QToken = match libos.pop(sockfd, None) {
            Ok(qt) => qt,
            Err(e) => {
                // Close socket on error.
                // FIXME: https://github.com/demikernel/demikernel/issues/633
                anyhow::bail!("pop() failed: {:?}", e)
            },
        };
        let (_, qr): (QDesc, OperationResult) = safe_wait2(&mut libos, qt)?;
        let bytes: DemiBuffer = match qr {
            OperationResult::Pop(_, bytes) => bytes,
            _ => {
                // Close socket on error.
                // FIXME: https://github.com/demikernel/demikernel/issues/633
                anyhow::bail!("pop() failed")
            },
        };

        // Push data.
        let qt: QToken = libos.pushto2(sockfd, &bytes, alice_addr).unwrap();
        let (_, qr): (QDesc, OperationResult) = safe_wait2(&mut libos, qt)?;
        match qr {
            OperationResult::Push => (),
            _ => {
                // Close socket on error.
                // FIXME: https://github.com/demikernel/demikernel/issues/633
                anyhow::bail!("push() failed")
            },
        }

        // Close connection.
        match libos.close(sockfd) {
            Ok(_) => Ok(()),
            Err(e) => anyhow::bail!("close() failed: {:?}", e),
        }
    });

    // It is safe to use unwrap here because there should not be any reason that we can't join the thread and if there
    // is, there is nothing to clean up here on the main thread.
    alice.join().unwrap()?;
    bob.join().unwrap()?;

    Ok(())
}

//======================================================================================================================
// Standalone Functions
//======================================================================================================================

/// Safe call to `wait2()`.
fn safe_wait2<const N: usize>(libos: &mut InetStack<N>, qt: QToken) -> Result<(QDesc, OperationResult)> {
    match libos.wait2(qt) {
        Ok((qd, qr)) => Ok((qd, qr)),
        Err(e) => {
            // Close socket on error.
            // FIXME: https://github.com/demikernel/demikernel/issues/633
            anyhow::bail!("operation failed: {:?}", e.cause)
        },
    }
}
