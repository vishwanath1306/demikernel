// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

//======================================================================================================================
// Imports
//======================================================================================================================

use crate::{
    catloop::{
        CatloopLibOS,
        DuplexPipe,
    },
    catmem::CatmemLibOS,
    demi_sgarray_t,
    runtime::{
        fail::Fail,
        types::{
            demi_opcode_t,
            demi_qresult_t,
        },
    },
    QToken,
};
use ::std::{
    cell::RefCell,
    future::Future,
    mem,
    net::{
        Ipv4Addr,
        SocketAddrV4,
    },
    pin::Pin,
    rc::Rc,
    slice,
    task::{
        Context,
        Poll,
    },
};

//======================================================================================================================
// Constants
//======================================================================================================================

/// Maximum number of connection attempts.
/// This was chosen arbitrarily.
const MAX_ACK_RECEIVED_ATTEMPTS: usize = 1024;

//======================================================================================================================
// Enumerations
//======================================================================================================================

/// Client-side states in the connection establishment protocol.
enum ClientState {
    InitiateConnectRequest {
        qt_rx: Option<QToken>,
    },
    ConnectRequestSent {
        qt_tx: QToken,
        qt_rx: Option<QToken>,
    },
    ConnectAckReceived {
        attempt: usize,
        qt_rx: QToken,
    },
    Connected {
        qt_tx: QToken,
        remote: SocketAddrV4,
        duplex_pipe: Rc<DuplexPipe>,
    },
}

//======================================================================================================================
// Structures
//======================================================================================================================

/// Descriptor for connect operation.
pub struct ConnectFuture {
    /// Underlying Catmem LibOS.
    catmem: Rc<RefCell<CatmemLibOS>>,
    /// Remote IPv4 address.
    ipv4: Ipv4Addr,
    /// Control duplex pipe used for establishing a the connection.
    control_duplex_pipe: Rc<DuplexPipe>,
    // State in the connection establishment protocol.
    state: ClientState,
}

//======================================================================================================================
// Associate Functions
//======================================================================================================================

/// Associate Functions for Connect Operation Descriptors
impl ConnectFuture {
    /// Creates a descriptor for a push operation.
    pub fn new(catmem: Rc<RefCell<CatmemLibOS>>, remote: SocketAddrV4) -> Result<Self, Fail> {
        let ipv4: &Ipv4Addr = remote.ip();
        let port: u16 = remote.port().into();
        let control_duplex_pipe: Rc<DuplexPipe> = Rc::new(DuplexPipe::open_duplex_pipe(catmem.clone(), ipv4, port)?);

        Ok(ConnectFuture {
            catmem,
            ipv4: ipv4.clone(),
            control_duplex_pipe,
            state: ClientState::InitiateConnectRequest { qt_rx: None },
        })
    }
}

//======================================================================================================================
// Trait Implementations
//======================================================================================================================

/// Future Trait Implementation for Connect Operation Descriptors
impl Future for ConnectFuture {
    type Output = Result<(SocketAddrV4, Rc<DuplexPipe>), Fail>;

    /// Polls the target [ConnectFuture].
    fn poll(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        let self_: &mut ConnectFuture = self.get_mut();

        // Poll Catmem LibOS to make progress on ongoing operations.
        self_.catmem.borrow_mut().poll();

        // Act according to the state in the connection establishment protocol.
        //
        //  CLIENT                                                       SERVER
        //    InitiateConnectRequest   --- ( msg: connect-request ) --->   ListenAndAccept
        //    ConnectRequestSent       <----- ( ack: port-number ) -----   Connect
        //    ConnectAckReceived                                           Connected
        //    Connected
        //
        match &self_.state {
            ClientState::InitiateConnectRequest { qt_rx } => setup(self_, ctx, *qt_rx),
            ClientState::ConnectRequestSent { qt_tx, qt_rx } => connect_request_sent(self_, ctx, *qt_tx, *qt_rx),
            ClientState::ConnectAckReceived { qt_rx, attempt } => connect_ack_received(self_, ctx, *qt_rx, *attempt),
            ClientState::Connected {
                qt_tx,
                remote,
                duplex_pipe,
            } => {
                if let Some(handle) = DuplexPipe::poll(&self_.catmem, *qt_tx)? {
                    // Retrieve operation result and check if it is what we expect.
                    let qr: demi_qresult_t = self_.catmem.borrow_mut().pack_result(handle, *qt_tx)?;
                    match qr.qr_opcode {
                        // We expect a successful completion for previous push().
                        demi_opcode_t::DEMI_OPC_PUSH => {},
                        // We may get some error.
                        demi_opcode_t::DEMI_OPC_FAILED => {
                            let cause: String = format!(
                                "failed to establish connection (qd={:?}, qt={:?}, errno={:?})",
                                qr.qr_qd, *qt_tx, qr.qr_ret
                            );
                            error!("poll(): {:?}", &cause);
                            return Poll::Ready(Err(Fail::new(qr.qr_ret as i32, &cause)));
                        },
                        // We do not expect anything else.
                        _ => {
                            // The following statement is unreachable because we have issued a pop operation.
                            // If we successfully complete a different operation, something really bad happen in the scheduler.
                            unreachable!("unexpected operation on control duplex pipe")
                        },
                    }
                    return Poll::Ready(Ok((*remote, duplex_pipe.clone())));
                }

                // Re-schedule co-routine for later execution.
                ctx.waker().wake_by_ref();
                return Poll::Pending;
            },
        }
    }
}

//======================================================================================================================
// Standalone Functions
//======================================================================================================================

/// Runs the "Initiate Connect Request" state in the connection establishment protocol.
fn setup(
    self_: &mut ConnectFuture,
    ctx: &mut Context<'_>,
    qt_rx: Option<QToken>,
) -> Poll<Result<(SocketAddrV4, Rc<DuplexPipe>), Fail>> {
    // Send connection request.
    let sga: demi_sgarray_t = CatloopLibOS::cook_magic_connect(&self_.catmem)?;
    let qt_tx: QToken = self_.control_duplex_pipe.push(&sga)?;
    self_.catmem.borrow_mut().free_sgarray(sga)?;

    // Transition to the next state in the connection establishment protocol.
    self_.state = ClientState::ConnectRequestSent { qt_tx, qt_rx };

    // Re-schedule co-routine for later execution.
    ctx.waker().wake_by_ref();
    return Poll::Pending;
}

/// Runs the "Connect Request Sent" state in the connection establishment protocol.
fn connect_request_sent(
    self_: &mut ConnectFuture,
    ctx: &mut Context<'_>,
    qt_tx: QToken,
    qt_rx: Option<QToken>,
) -> Poll<Result<(SocketAddrV4, Rc<DuplexPipe>), Fail>> {
    // Check if connection request was sent.
    if let Some(handle) = DuplexPipe::poll(&self_.catmem, qt_tx)? {
        // Retrieve operation result and check if it is what we expect.
        let qr: demi_qresult_t = self_.catmem.borrow_mut().pack_result(handle, qt_tx)?;
        match qr.qr_opcode {
            // We expect a successful completion for previous push().
            demi_opcode_t::DEMI_OPC_PUSH => {},
            // We may get some error.
            demi_opcode_t::DEMI_OPC_FAILED => {
                let cause: String = format!(
                    "failed to establish connection (qd={:?}, qt={:?}, errno={:?})",
                    qr.qr_qd, qt_tx, qr.qr_ret
                );
                error!("connect_request_sent(): {:?}", &cause);
                return Poll::Ready(Err(Fail::new(qr.qr_ret as i32, &cause)));
            },
            // We do not expect anything else.
            _ => {
                // The following statement is unreachable because we have issued a pop operation.
                // If we successfully complete a different operation, something really bad happen in the scheduler.
                unreachable!("unexpected operation on control duplex pipe")
            },
        }

        // Issue receive operation to wait for connect request ack.
        let size: usize = mem::size_of::<u16>();
        let qt_rx: QToken = if let Some(qt_rx) = qt_rx {
            qt_rx
        } else {
            warn!("connect_request_sent(): qt_rx is None, allocating new qtoken");
            self_.control_duplex_pipe.pop(Some(size))?
        };

        // Transition to the next state in the connection establishment protocol.
        self_.state = ClientState::ConnectAckReceived { qt_rx, attempt: 0 };
    }

    // Re-schedule co-routine for later execution.
    ctx.waker().wake_by_ref();
    return Poll::Pending;
}

/// Runs the "Connect Ack Received" state in the connection establishment protocol.
fn connect_ack_received(
    self_: &mut ConnectFuture,
    ctx: &mut Context<'_>,
    qt_rx: QToken,
    attempt: usize,
) -> Poll<Result<(SocketAddrV4, Rc<DuplexPipe>), Fail>> {
    // Check if we received a connect request ack.
    if let Some(handle) = DuplexPipe::poll(&self_.catmem, qt_rx)? {
        // Retrieve operation result and check if it is what we expect.
        let qr: demi_qresult_t = self_.catmem.borrow_mut().pack_result(handle, qt_rx)?;
        match qr.qr_opcode {
            // We expect a successful completion for previous pop().
            demi_opcode_t::DEMI_OPC_POP => {},
            // We may get some error.
            demi_opcode_t::DEMI_OPC_FAILED => {
                let cause: String = format!(
                    "failed to establish connection (qd={:?}, qt={:?}, errno={:?})",
                    qr.qr_qd, qt_rx, qr.qr_ret
                );
                error!("connect_ack_received(): {:?}", &cause);
                return Poll::Ready(Err(Fail::new(qr.qr_ret as i32, &cause)));
            },
            // We do not expect anything else.
            _ => {
                // The following statement is unreachable because we have issued a pop operation.
                // If we successfully complete a different operation, something really bad happen in the scheduler.
                unreachable!("unexpected operation on control duplex pipe")
            },
        }

        // Extract scatter-gather array from operation result.
        let sga: demi_sgarray_t = unsafe { qr.qr_value.sga };

        // Extract port number.
        let port: u16 = {
            let port: Result<u16, Fail> = extract_port_number(&sga);
            self_.catmem.borrow_mut().free_sgarray(sga)?;
            self_.control_duplex_pipe.shutdown()?;
            port?
        };

        // Open underlying pipes.
        let remote: SocketAddrV4 = SocketAddrV4::new(self_.ipv4, port);
        let duplex_pipe: Rc<DuplexPipe> =
            Rc::new(DuplexPipe::open_duplex_pipe(self_.catmem.clone(), &self_.ipv4, port)?);

        let sga: demi_sgarray_t = CatloopLibOS::cook_magic_connect(&self_.catmem)?;
        let qt_tx: QToken = duplex_pipe.push(&sga)?;
        self_.catmem.borrow_mut().free_sgarray(sga)?;

        // Transition to the next state in the connection establishment protocol.
        self_.state = ClientState::Connected {
            qt_tx,
            remote,
            duplex_pipe,
        };
    } else {
        if attempt > MAX_ACK_RECEIVED_ATTEMPTS {
            // Connection timeout, retry.
            debug!(
                "connect_ack_received(): connection timeout, retrying (qt_rx={:?})",
                qt_rx
            );
            self_.state = ClientState::InitiateConnectRequest { qt_rx: Some(qt_rx) };
        } else {
            self_.state = ClientState::ConnectAckReceived {
                qt_rx,
                attempt: attempt + 1,
            };
        }
    }

    // Re-schedule co-routine for later execution.
    ctx.waker().wake_by_ref();
    return Poll::Pending;
}

/// Extracts port number from connect request ack message.
fn extract_port_number(sga: &demi_sgarray_t) -> Result<u16, Fail> {
    let ptr: *mut u8 = sga.sga_segs[0].sgaseg_buf as *mut u8;
    let len: usize = sga.sga_segs[0].sgaseg_len as usize;
    if len != 2 {
        let e: Fail = Fail::new(libc::EAGAIN, "hashsake failed");
        error!("failed to establish connection ({:?})", e);
        return Err(e);
    }
    let slice: &mut [u8] = unsafe { slice::from_raw_parts_mut(ptr, len) };
    let array: [u8; 2] = [slice[0], slice[1]];
    Ok(u16::from_ne_bytes(array))
}
