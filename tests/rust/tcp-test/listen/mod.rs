// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

//======================================================================================================================
// Imports
//======================================================================================================================

use anyhow::Result;
use demikernel::{
    runtime::types::demi_opcode_t,
    LibOS,
    QDesc,
    QToken,
};
use std::{
    net::SocketAddrV4,
    time::Duration,
};

//======================================================================================================================
// Constants
//======================================================================================================================

#[cfg(target_os = "windows")]
pub const AF_INET: i32 = windows::Win32::Networking::WinSock::AF_INET.0 as i32;

#[cfg(target_os = "windows")]
pub const SOCK_STREAM: i32 = windows::Win32::Networking::WinSock::SOCK_STREAM as i32;

#[cfg(target_os = "linux")]
pub const AF_INET: i32 = libc::AF_INET;

#[cfg(target_os = "linux")]
pub const SOCK_STREAM: i32 = libc::SOCK_STREAM;

//======================================================================================================================
// Standalone Functions
//======================================================================================================================

/// Drives integration tests for listen() on TCP sockets.
pub fn run(
    libos: &mut LibOS,
    local: &SocketAddrV4,
    remote: &SocketAddrV4,
) -> Vec<(String, String, Result<(), anyhow::Error>)> {
    let mut result: Vec<(String, String, Result<(), anyhow::Error>)> = Vec::new();

    crate::collect!(result, crate::test!(listen_invalid_queue_descriptor(libos)));
    crate::collect!(result, crate::test!(listen_unbound_socket(libos)));
    crate::collect!(result, crate::test!(listen_bound_socket(libos, local)));
    crate::collect!(result, crate::test!(listen_large_backlog_length(libos, local)));
    crate::collect!(result, crate::test!(listen_invalid_zero_backlog_length(libos, local)));
    crate::collect!(result, crate::test!(listen_listening_socket(libos, local)));
    crate::collect!(result, crate::test!(listen_connecting_socket(libos, local, remote)));
    crate::collect!(result, crate::test!(listen_accepting_socket(libos, local)));
    crate::collect!(result, crate::test!(listen_closed_socket(libos, local)));

    result
}

/// Attempts to listen for connections on an invalid queue descriptor.
fn listen_invalid_queue_descriptor(libos: &mut LibOS) -> Result<()> {
    // Fail to listen().
    match libos.listen(QDesc::from(0), 8) {
        Err(e) if e.errno == libc::EBADF => Ok(()),
        Err(e) => anyhow::bail!("listen() failed with {}", e),
        Ok(()) => anyhow::bail!("listen() on an invalid queue descriptor should fail"),
    }
}

/// Attempts to listen for connections on a TCP socket that is not bound.
fn listen_unbound_socket(libos: &mut LibOS) -> Result<()> {
    // Create an unbound socket.
    let sockqd: QDesc = libos.socket(AF_INET, SOCK_STREAM, 0)?;

    // Fail to listen().
    match libos.listen(sockqd, 16) {
        Err(e) if e.errno == libc::EDESTADDRREQ => (),
        Err(e) => anyhow::bail!("listen() failed with {}", e),
        Ok(()) => anyhow::bail!("listen() on a socket that is not bound should fail"),
    };

    // Succeed to close socket.
    libos.close(sockqd)?;

    Ok(())
}

/// Attempts to listen for connections on a TCP socket that is bound.
fn listen_bound_socket(libos: &mut LibOS, local: &SocketAddrV4) -> Result<()> {
    // Create a bound socket.
    let sockqd: QDesc = libos.socket(AF_INET, SOCK_STREAM, 0)?;
    libos.bind(sockqd, local.to_owned())?;

    // Succeed to listen().
    libos.listen(sockqd, 16)?;

    // Succeed to close socket.
    libos.close(sockqd)?;

    Ok(())
}

/// Attempts to listen for connections on a TCP socket with a zero backlog length.
fn listen_invalid_zero_backlog_length(libos: &mut LibOS, local: &SocketAddrV4) -> Result<()> {
    // Create a bound socket.
    let sockqd: QDesc = libos.socket(AF_INET, SOCK_STREAM, 0)?;
    libos.bind(sockqd, local.to_owned())?;

    // Backlog length.
    let backlog: usize = 0;

    // Succeed to listen().
    libos.listen(sockqd, backlog)?;

    // Succeed to close socket.
    libos.close(sockqd)?;

    Ok(())
}

/// Attempts to listen for connections on a TCP socket with a large backlog length.
fn listen_large_backlog_length(libos: &mut LibOS, local: &SocketAddrV4) -> Result<()> {
    // Create a bound socket.
    let sockqd: QDesc = libos.socket(AF_INET, SOCK_STREAM, 0)?;
    libos.bind(sockqd, local.to_owned())?;

    // Backlog length.
    let backlog: usize = (libc::SOMAXCONN + 1) as usize;

    // Succeed to listen().
    libos.listen(sockqd, backlog)?;

    // Succeed to close socket.
    libos.close(sockqd)?;

    Ok(())
}

/// Attempts to listen for connections on a TCP socket that is already listening for connections.
fn listen_listening_socket(libos: &mut LibOS, local: &SocketAddrV4) -> Result<()> {
    // Create a bound socket.
    let sockqd: QDesc = libos.socket(AF_INET, SOCK_STREAM, 0)?;
    libos.bind(sockqd, local.to_owned())?;

    // Succeed to listen().
    libos.listen(sockqd, 16)?;

    // Fail to listen().
    match libos.listen(sockqd, 16) {
        Err(e) if e.errno == libc::EADDRINUSE => (),
        Err(e) => anyhow::bail!("listen() failed with {}", e),
        Ok(()) => anyhow::bail!("listen() on a socket that is already listening should fail"),
    };

    // Succeed to close socket.
    libos.close(sockqd)?;

    Ok(())
}

/// Attempts to listen for connections on a TCP socket that is connecting.
fn listen_connecting_socket(libos: &mut LibOS, local: &SocketAddrV4, remote: &SocketAddrV4) -> Result<()> {
    // Create a connecting socket.
    let sockqd: QDesc = libos.socket(AF_INET, SOCK_STREAM, 0)?;
    libos.bind(sockqd, local.to_owned())?;
    let qt: QToken = libos.connect(sockqd, remote.to_owned())?;
    let mut connect_finished: bool = false;

    // Poll once to ensure that the connect() co-routine runs.
    match libos.wait(qt, Some(Duration::from_micros(0))) {
        Err(e) if e.errno == libc::ETIMEDOUT => {},
        Ok(qr) if qr.qr_opcode == demi_opcode_t::DEMI_OPC_FAILED && qr.qr_ret == libc::ECONNREFUSED as i64 => {
            connect_finished = true
        },
        // If completes successfully, something has gone wrong.
        Ok(qr) if qr.qr_opcode == demi_opcode_t::DEMI_OPC_CONNECT && qr.qr_ret == 0 => {
            anyhow::bail!("connect() should not succeed because remote does not exist")
        },
        Ok(_) => anyhow::bail!("wait() should not succeed"),
        Err(_) => anyhow::bail!("wait() should timeout"),
    }

    // Fail to listen().
    // TODO: Not sure if we should be able to listen after a failed connect().
    if connect_finished {
        // Succeed to listen().
        libos.listen(sockqd, 16)?;
    } else {
        match libos.listen(sockqd, 16) {
            Err(e) if e.errno == libc::EADDRINUSE => (),
            Err(e) => anyhow::bail!("listen() failed with {}", e),
            Ok(()) => anyhow::bail!("listen() on a socket that is connecting should fail"),
        };
    }

    // Succeed to close socket.
    libos.close(sockqd)?;

    if !connect_finished {
        // Poll again to check that the connect() co-routine returns an err, either canceled or refused.
        match libos.wait(qt, Some(Duration::from_micros(0))) {
            Ok(qr) if qr.qr_opcode == demi_opcode_t::DEMI_OPC_FAILED && qr.qr_ret == libc::ECANCELED as i64 => {},
            Ok(qr) if qr.qr_opcode == demi_opcode_t::DEMI_OPC_FAILED && qr.qr_ret == libc::ECONNREFUSED as i64 => {},
            // If connect() completes successfully, something has gone wrong.
            Ok(qr) if qr.qr_opcode == demi_opcode_t::DEMI_OPC_CONNECT && qr.qr_ret == 0 => {
                anyhow::bail!("connect() should not succeed because remote does not exist")
            },
            Ok(_) => anyhow::bail!("wait() should return an error on connect() after close()"),
            Err(_) => anyhow::bail!("wait() should not time out"),
        }
    }

    Ok(())
}

/// Attempts to listen for connections on a TCP socket that is accepting connections.
fn listen_accepting_socket(libos: &mut LibOS, local: &SocketAddrV4) -> Result<()> {
    // Create an accepting socket.
    let sockqd: QDesc = libos.socket(AF_INET, SOCK_STREAM, 0)?;
    libos.bind(sockqd, local.to_owned())?;
    libos.listen(sockqd, 16)?;
    let qt: QToken = libos.accept(sockqd)?;

    // Poll once to ensure that the accept() co-routine runs.
    match libos.wait(qt, Some(Duration::from_micros(0))) {
        Err(e) if e.errno == libc::ETIMEDOUT => {},
        // If we found a connection to accept, something has gone wrong.
        Ok(qr) if qr.qr_opcode == demi_opcode_t::DEMI_OPC_ACCEPT && qr.qr_ret == 0 => {
            anyhow::bail!("accept() should not succeed because remote should not be connecting")
        },
        Ok(_) => anyhow::bail!("wait() should not succeed"),
        Err(_) => anyhow::bail!("wait() should timeout"),
    }

    // Fail to listen().
    match libos.listen(sockqd, 16) {
        Err(e) if e.errno == libc::EADDRINUSE => (),
        Err(e) => anyhow::bail!("listen() failed with {}", e),
        Ok(()) => anyhow::bail!("listen() on a socket that is accepting connections should fail"),
    };

    // Succeed to close socket.
    libos.close(sockqd)?;

    // Poll again to check that the qtoken returns an err.
    match libos.wait(qt, Some(Duration::from_micros(0))) {
        Ok(qr) if qr.qr_opcode == demi_opcode_t::DEMI_OPC_FAILED && qr.qr_ret == libc::ECANCELED as i64 => {},
        // If we found a connection to accept, something has gone wrong.
        Ok(qr) if qr.qr_opcode == demi_opcode_t::DEMI_OPC_ACCEPT && qr.qr_ret == 0 => {
            anyhow::bail!("accept() should not succeed because remote should not be connecting")
        },
        Ok(_) => anyhow::bail!("wait() should succeed with an error on accept() after close()"),
        Err(_) => anyhow::bail!("wait() should not time out"),
    }

    Ok(())
}

/// Attempts to listen for connections on a TCP socket that is closed.
fn listen_closed_socket(libos: &mut LibOS, local: &SocketAddrV4) -> Result<()> {
    // Create a bound socket.
    let sockqd: QDesc = libos.socket(AF_INET, SOCK_STREAM, 0)?;
    libos.bind(sockqd, local.to_owned())?;

    // Succeed to listen().
    libos.listen(sockqd, 16)?;

    // Succeed to close socket.
    libos.close(sockqd)?;

    // Fail to listen().
    match libos.listen(sockqd, 16) {
        Err(e) if e.errno == libc::EBADF => Ok(()),
        Err(e) => anyhow::bail!("listen() failed with {}", e),
        Ok(()) => anyhow::bail!("listen() on a socket that is closed should fail"),
    }
}
