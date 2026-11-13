// SPDX-License-Identifier: Apache-2.0

#![cfg(all(feature = "fusedev", target_os = "linux"))]

use std::fs::File;
use std::io::{ErrorKind, Read, Write};
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::sync::mpsc;
use std::time::Duration;

use fuse_backend_rs::transport::{Error, FuseSession};
use nix::fcntl::{fcntl, FcntlArg, OFlag};

#[test]
fn imported_file_is_nonblocking_and_preserves_flags() {
    let (socket, _peer) = UnixStream::pair().unwrap();
    let fd = socket.as_raw_fd();
    let flags = OFlag::from_bits_truncate(fcntl(fd, FcntlArg::F_GETFL).unwrap()) | OFlag::O_APPEND;
    fcntl(fd, FcntlArg::F_SETFL(flags)).unwrap();
    let descriptor_flags = fcntl(fd, FcntlArg::F_GETFD).unwrap();

    let session = FuseSession::from_file(File::from(OwnedFd::from(socket))).unwrap();
    let mut file = session.get_fuse_file().unwrap();
    let actual_flags =
        OFlag::from_bits_truncate(fcntl(file.as_raw_fd(), FcntlArg::F_GETFL).unwrap());
    assert_eq!(actual_flags, flags | OFlag::O_NONBLOCK);
    assert_eq!(
        fcntl(file.as_raw_fd(), FcntlArg::F_GETFD).unwrap(),
        descriptor_flags
    );
    assert_eq!(
        file.read(&mut [0]).unwrap_err().kind(),
        ErrorKind::WouldBlock
    );
}

#[test]
fn imported_session_rejects_mount_without_consuming_file() {
    let (socket, mut peer) = UnixStream::pair().unwrap();
    let mut session = FuseSession::from_file(File::from(OwnedFd::from(socket))).unwrap();

    let error = session.mount().unwrap_err();
    assert!(
        matches!(error, Error::SessionFailure(ref message) if message.contains("does not manage")),
        "{}",
        error
    );
    peer.write_all(b"x").unwrap();
    let mut buf = [0];
    session
        .get_fuse_file()
        .unwrap()
        .read_exact(&mut buf)
        .unwrap();
    assert_eq!(&buf, b"x");
}

#[test]
fn imported_session_rejects_umount_without_consuming_file() {
    let (socket, mut peer) = UnixStream::pair().unwrap();
    let mut session = FuseSession::from_file(File::from(OwnedFd::from(socket))).unwrap();

    let error = session.umount().unwrap_err();
    assert!(
        matches!(error, Error::SessionFailure(ref message) if message.contains("does not manage")),
        "{}",
        error
    );
    peer.write_all(b"x").unwrap();
    let mut buf = [0];
    session
        .get_fuse_file()
        .unwrap()
        .read_exact(&mut buf)
        .unwrap();
    assert_eq!(&buf, b"x");
}

#[test]
fn imported_session_wakes_all_channels() {
    let (socket, _peer) = UnixStream::pair().unwrap();
    let session = FuseSession::from_file(File::from(OwnedFd::from(socket))).unwrap();
    let channels = [
        session.new_channel().unwrap(),
        session.new_channel().unwrap(),
    ];
    let (send, recv) = mpsc::channel();
    let workers: Vec<_> = IntoIterator::into_iter(channels)
        .map(|mut channel| {
            let send = send.clone();
            std::thread::spawn(move || {
                send.send(channel.get_request().unwrap().is_none()).unwrap();
            })
        })
        .collect();

    session.wake().unwrap();
    for _ in &workers {
        assert!(recv.recv_timeout(Duration::from_secs(5)).unwrap());
    }
    for worker in workers {
        worker.join().unwrap();
    }
}

#[test]
fn dropping_imported_session_closes_file() {
    let (socket, mut peer) = UnixStream::pair().unwrap();
    peer.set_nonblocking(true).unwrap();
    let session = FuseSession::from_file(File::from(OwnedFd::from(socket))).unwrap();

    drop(session);
    assert_eq!(peer.read(&mut [0]).unwrap(), 0);
}

#[test]
fn imported_channel_keeps_file_open_until_dropped() {
    let (socket, mut peer) = UnixStream::pair().unwrap();
    peer.set_nonblocking(true).unwrap();
    let session = FuseSession::from_file(File::from(OwnedFd::from(socket))).unwrap();
    let channel = session.new_channel().unwrap();

    drop(session);
    assert_eq!(
        peer.read(&mut [0]).unwrap_err().kind(),
        ErrorKind::WouldBlock
    );
    drop(channel);
    assert_eq!(peer.read(&mut [0]).unwrap(), 0);
}
