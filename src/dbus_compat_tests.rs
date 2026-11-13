//! Hostile/limited authentication peers must not bypass the upstream checks.
use std::{
    io::{BufRead, BufReader, Read, Write},
    os::{fd::OwnedFd, unix::net::UnixStream},
    process::Command,
    time::Duration,
};

#[derive(Debug)]
struct Reader(UnixStream);

#[zbus::export::async_trait::async_trait]
impl zbus::connection::socket::ReadHalf for Reader {
    async fn recvmsg(&mut self, buffer: &mut [u8]) -> std::io::Result<(usize, Vec<OwnedFd>)> {
        self.0.read(buffer).map(|n| (n, vec![]))
    }
}

fn line(reader: &mut BufReader<UnixStream>) -> Vec<u8> {
    let mut bytes = vec![];
    reader.read_until(b'\n', &mut bytes).unwrap();
    bytes
}

#[test]
fn sequential_auth_rejects_invalid_peers_and_honors_fd_refusal() {
    const TEST: &str =
        "dbus_compat_tests::sequential_auth_rejects_invalid_peers_and_honors_fd_refusal";
    if std::env::var_os("LSE_TEST_SASL_PEER").is_none() {
        let result = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture"])
            .env("LSE_TEST_SASL_PEER", "1")
            .env("FLATPAK_ID", crate::desktop::APP_ID)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        return;
    }
    use zbus::connection::socket::ReadHalf;
    for answer in [
        "REJECTED EXTERNAL",
        "OK invalid-guid",
        "OK 11111111111111111111111111111111",
        "OK 00000000000000000000000000000000",
    ] {
        let root = tempfile::tempdir().unwrap();
        let socket_path = root.path().join("bus");
        let listener = std::os::unix::net::UnixListener::bind(&socket_path).unwrap();
        let fd_refused = answer.ends_with("00000000000000000000000000000000");
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut reader = BufReader::with_capacity(1, stream);
            let auth = line(&mut reader);
            assert!(auth.starts_with(b"\0AUTH EXTERNAL ") && auth.ends_with(b"\r\n"));
            reader
                .get_mut()
                .write_all(format!("{answer}\r\n").as_bytes())
                .unwrap();
            if fd_refused {
                assert_eq!(line(&mut reader), b"NEGOTIATE_UNIX_FD\r\n");
                reader
                    .get_mut()
                    .write_all(b"ERROR FD passing not supported\r\n")
                    .unwrap();
                assert_eq!(line(&mut reader), b"BEGIN\r\n");
                assert!(reader.buffer().is_empty());
                let mut wire = Reader(reader.into_inner());
                let hello =
                    zbus::block_on(wire.receive_message(0, &mut vec![], &mut vec![])).unwrap();
                assert_eq!(hello.header().member().unwrap().as_str(), "Hello");
                let reply = zbus::Message::method_return(&hello.header())
                    .build(&":1.42")
                    .unwrap();
                wire.0.write_all(reply.data()).unwrap();
                let mut unexpected = vec![];
                wire.0.read_to_end(&mut unexpected).unwrap();
                assert!(
                    unexpected.is_empty(),
                    "an unnegotiated FD must never be sent"
                );
            } else {
                // Includes bytes already pipelined in the initial AUTH write.
                let mut unexpected = vec![];
                reader.read_to_end(&mut unexpected).unwrap();
                assert!(
                    unexpected.is_empty(),
                    "failed AUTH/GUID must not send later commands"
                );
            }
        });
        let address = format!(
            "unix:path={},guid=00000000000000000000000000000000",
            socket_path.display()
        );
        zbus::block_on(async {
            let result = zbus::connection::Builder::address(address.as_str())
                .method_timeout(Duration::from_secs(2))
                .build()
                .await;
            if fd_refused {
                let connection = result.unwrap();
                let file = tempfile::tempfile().unwrap();
                let result = connection
                    .call_method(
                        Some("org.example.Portal"),
                        "/",
                        Some("org.example.Portal"),
                        "OpenFile",
                        &(zbus::Fd::from(&file),),
                    )
                    .await;
                assert!(matches!(result, Err(zbus::Error::Unsupported)));
                drop(connection);
            } else {
                assert!(result.is_err(), "invalid peer must fail authentication");
            }
        });
        server.join().unwrap();
    }
}
