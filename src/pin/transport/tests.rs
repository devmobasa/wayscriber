use super::*;
use std::os::unix::fs::PermissionsExt;
use std::sync::mpsc;

fn paths() -> (crate::test_temp::TempDir, PinRuntimePaths) {
    let root = crate::test_temp::tempdir().unwrap();
    std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let directory = root.path().join(PIN_DIR);
    std::fs::create_dir(&directory).unwrap();
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).unwrap();
    let paths = PinRuntimePaths {
        socket: directory.join(PIN_SOCKET),
        lock: directory.join(PIN_LOCK),
        start_lock: directory.join(PIN_START_LOCK),
    };
    (root, paths)
}

fn socket_pair() -> (OwnedFd, OwnedFd) {
    let mut pair = [0; 2];
    // SAFETY: pair has storage for the two returned descriptors.
    assert_eq!(
        unsafe {
            libc::socketpair(
                libc::AF_UNIX,
                libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC,
                0,
                pair.as_mut_ptr(),
            )
        },
        0
    );
    // SAFETY: socketpair returned two new owned descriptors.
    unsafe { (OwnedFd::from_raw_fd(pair[0]), OwnedFd::from_raw_fd(pair[1])) }
}

#[test]
fn v1_hello_and_sealed_create_round_trip() {
    let (_root, paths) = paths();
    let lock = HostLock::try_acquire(&paths).unwrap().unwrap();
    let listener = PinListener::bind(&paths, &lock).unwrap();
    let socket = paths.socket().to_owned();
    let (tx, rx) = mpsc::channel();
    let client = std::thread::spawn(move || {
        let mut connection = PinConnection::connect(&socket, Duration::from_secs(1)).unwrap();
        connection.negotiate_client().unwrap();
        let wire = PinCreateWire {
            request_id: super::super::PinRequestId::new(1).unwrap(),
            png_length: 8,
            width: 1,
            height: 1,
            output: super::super::PinOutputHint::new(
                "DP-1".into(),
                100,
                100,
                1,
                super::super::PinOutputTransform::Normal,
            )
            .unwrap(),
            placement: super::super::PinPlacementHint::new(1.0, 2.0, 3.0, 4.0).unwrap(),
        };
        let response = connection.create_client(wire, b"12345678").unwrap();
        tx.send(response).unwrap();
    });
    let mut connection = listener.accept().unwrap().unwrap();
    assert!(matches!(
        connection.receive().unwrap(),
        ReceivedPacket::Hello { version: 1 }
    ));
    connection.send_hello(1).unwrap();
    let ReceivedPacket::Create(pending) = connection.receive().unwrap() else {
        panic!("expected create");
    };
    let (wire, bytes) = pending.read_png().unwrap();
    assert_eq!(bytes, b"12345678");
    connection
        .send_response(PinCreateResponse::Ready {
            request_id: wire.request_id,
            pin_id: super::super::PinId::new(2).unwrap(),
        })
        .unwrap();
    assert!(matches!(
        rx.recv().unwrap(),
        PinCreateResponse::Ready { .. }
    ));
    client.join().unwrap();
}

#[test]
fn unsealed_memfd_is_rejected_before_pending_create() {
    // SAFETY: the name is valid and flags intentionally omit sealing.
    let raw = unsafe { libc::memfd_create(c"unsealed-test".as_ptr(), libc::MFD_CLOEXEC) };
    assert!(raw >= 0);
    // SAFETY: memfd_create returned a new owned descriptor.
    let descriptor = unsafe { OwnedFd::from_raw_fd(raw) };
    // SAFETY: one valid byte is written to the live descriptor.
    assert_eq!(unsafe { libc::write(raw, b"x".as_ptr().cast(), 1) }, 1);
    assert!(crate::unix_transport::validate_sealed_memfd(&descriptor, 1).is_err());
}

#[test]
fn host_ownership_is_independent_of_starter_serialization() {
    let (_root, paths) = paths();
    let starter = StarterLock::try_acquire(&paths).unwrap().unwrap();
    let host = HostLock::acquire_for(&paths, Duration::from_secs(1)).unwrap();
    assert!(HostLock::try_acquire(&paths).unwrap().is_none());
    drop(starter);
    assert!(HostLock::try_acquire(&paths).unwrap().is_none());
    drop(host);
    assert!(HostLock::try_acquire(&paths).unwrap().is_some());
}

#[test]
fn bind_under_lock_refuses_non_socket_stale_entry() {
    let (_root, paths) = paths();
    let lock = HostLock::try_acquire(&paths).unwrap().unwrap();
    std::fs::write(paths.socket(), b"do not unlink").unwrap();
    assert!(PinListener::bind(&paths, &lock).is_err());
    assert_eq!(std::fs::read(paths.socket()).unwrap(), b"do not unlink");
}

#[test]
fn wrong_descriptor_count_and_oversized_packets_are_rejected() {
    let (sender, receiver) = socket_pair();
    crate::unix_transport::send_packet(
        sender.as_raw_fd(),
        &vec![b'x'; MAX_PIN_PACKET_BYTES + 1],
        &[],
    )
    .unwrap();
    assert!(
        crate::unix_transport::recv_packet(receiver.as_raw_fd(), MAX_PIN_PACKET_BYTES, 1,).is_err()
    );

    let (sender, receiver) = socket_pair();
    let first = crate::unix_transport::sealed_memfd(c"first", b"x").unwrap();
    let second = crate::unix_transport::sealed_memfd(c"second", b"x").unwrap();
    crate::unix_transport::send_packet(
        sender.as_raw_fd(),
        b"{}",
        &[first.as_raw_fd(), second.as_raw_fd()],
    )
    .unwrap();
    assert!(
        crate::unix_transport::recv_packet(receiver.as_raw_fd(), MAX_PIN_PACKET_BYTES, 1,).is_err()
    );
}

#[test]
fn peer_authorization_requires_exact_effective_uid() {
    assert!(authorize_peer_uid(1000, 1000));
    assert!(!authorize_peer_uid(1001, 1000));
}

#[test]
fn listener_requires_the_lock_for_its_exact_runtime_paths() {
    let (_first_root, first) = paths();
    let (_second_root, second) = paths();
    let lock = HostLock::try_acquire(&first).unwrap().unwrap();

    assert!(PinListener::bind(&second, &lock).is_err());
    assert!(!second.socket().exists());
}

#[test]
fn unknown_protocol_version_gets_a_terminal_refusal() {
    let (_root, paths) = paths();
    let lock = HostLock::try_acquire(&paths).unwrap().unwrap();
    let listener = PinListener::bind(&paths, &lock).unwrap();
    let socket = paths.socket().to_owned();
    let client = std::thread::spawn(move || {
        let connection = PinConnection::connect(&socket, Duration::from_secs(1)).unwrap();
        send_client_packet(
            connection.as_raw_fd(),
            &PinClientPacket::Hello { version: 99 },
            &[],
        )
        .unwrap();
        recv_host_packet(connection.as_raw_fd()).unwrap()
    });
    let mut connection = listener.accept().unwrap().unwrap();
    let ReceivedPacket::Hello { version } = connection.receive().unwrap() else {
        panic!("expected hello");
    };
    assert!(!connection.send_hello(version).unwrap());
    assert_eq!(
        client.join().unwrap(),
        PinHostPacket::UnsupportedVersion {
            requested: 99,
            supported: PIN_PROTOCOL_VERSION,
        }
    );
}

#[test]
fn concurrent_starters_have_exactly_one_serialization_owner() {
    let (_root, paths) = paths();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
    let (sender, receiver) = mpsc::channel();
    let threads: Vec<_> = (0..2)
        .map(|_| {
            let paths = paths.clone();
            let barrier = barrier.clone();
            let sender = sender.clone();
            std::thread::spawn(move || {
                barrier.wait();
                let lock = StarterLock::try_acquire(&paths).unwrap();
                sender.send(lock.is_some()).unwrap();
                barrier.wait();
                drop(lock);
            })
        })
        .collect();
    barrier.wait();
    let owned = [receiver.recv().unwrap(), receiver.recv().unwrap()];
    assert_eq!(owned.into_iter().filter(|owned| *owned).count(), 1);
    barrier.wait();
    for thread in threads {
        thread.join().unwrap();
    }
}
