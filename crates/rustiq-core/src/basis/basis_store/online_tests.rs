use super::*;
use futures_lite::future::block_on;
use std::io::{BufRead, Write};
use std::net::TcpListener;
use std::thread;
use std::time::{Duration, Instant};

fn serve(responses: Vec<String>) -> (Url, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
    let handle = thread::spawn(move || {
        let mut requests = Vec::new();
        for response in responses {
            let deadline = Instant::now() + Duration::from_secs(10);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline, "HTTP client did not connect");
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => panic!("{error}"),
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut reader = io::BufReader::new(&mut stream);
            let mut request = String::new();
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" || line.is_empty() {
                    break;
                }
                request.push_str(&line);
            }
            requests.push(request);
            stream.write_all(response.as_bytes()).unwrap();
        }
        requests
    });
    (url, handle)
}

fn ok(body: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

#[test]
fn downloads_sync_and_async_without_runtime() {
    let body = include_str!("../../../tests/data/sto-3g.json");
    for asynchronous in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let mut store = BasisStore::new(&directory.path().join("nested"));
        let (url, server) = serve(vec![ok(body)]);
        store.url = url;
        let mut progress = Vec::new();
        if asynchronous {
            block_on(store.download("cc-pV/DZ", &mut |current, total| {
                progress.push((current, total));
            }))
            .unwrap();
            assert_eq!(
                progress.last(),
                Some(&(body.len() as u64, Some(body.len() as u64)))
            );
            assert!(progress.windows(2).all(|pair| pair[0].0 < pair[1].0));
        } else {
            store.download_sync("cc-pV/DZ").unwrap();
        }
        assert_eq!(
            fs::read(store.path().join("cc-pv_sl_dz.json")).unwrap(),
            body.as_bytes()
        );
        assert!(store.get("cc-pV/DZ").unwrap().is_some());
        let requests = server.join().unwrap();
        assert!(requests[0].starts_with("GET /api/basis/cc-pV%2FDZ/format/json "));
        assert!(requests[0]
            .to_ascii_lowercase()
            .contains(&format!("user-agent: {}", USER_AGENT.to_ascii_lowercase())));
    }
}

#[test]
fn lists_metadata_sync_and_async_and_rejects_invalid_json() {
    let body = r#"{"sto-3g":{"basename":"sto-3g","description":"Test","display_name":"STO-3G","family":"sto","function_types":[],"latest_version":"1","notes_exist":[],"other_names":[],"relpath":"","role":"orbital","tags":[],"versions":{}}}"#;
    for asynchronous in [false, true] {
        for (body, valid) in [(body, true), ("invalid", false)] {
            let directory = tempfile::tempdir().unwrap();
            let mut store = BasisStore::new(&directory);
            let (url, server) = serve(vec![ok(body)]);
            store.url = url;
            let result = if asynchronous {
                block_on(store.list_online())
            } else {
                store.list_online_sync()
            };
            if valid {
                let entries = result.unwrap();
                assert_eq!(entries.len(), 1);
                assert_eq!(
                    entries[&BasisId::new("sto-3g").unwrap()].display_name,
                    "STO-3G"
                );
            } else {
                assert!(matches!(result, Err(DownloadParseError::Serde(_))));
            }
            assert!(server.join().unwrap()[0].starts_with("GET /api/metadata "));
        }
    }
}

#[test]
fn failed_download_preserves_destination() {
    for asynchronous in [false, true] {
        for response in [
            "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            "HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\npartial",
        ] {
            let directory = tempfile::tempdir().unwrap();
            let mut store = BasisStore::new(&directory);
            let path = directory.path().join("test.json");
            fs::write(&path, "original").unwrap();
            let (url, server) = serve(vec![response.to_owned()]);
            store.url = url;
            let result = if asynchronous {
                block_on(store.download("test", &mut |_, _| {}))
            } else {
                store.download_sync("test")
            };
            let error = result.unwrap_err();
            if response.contains("404") {
                assert!(matches!(
                    error,
                    DownloadSaveError::Http(HttpError::Status(404))
                ));
            }
            server.join().unwrap();
            assert_eq!(fs::read(path).unwrap(), b"original");
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        }
    }
}

#[test]
fn follows_redirects_and_streams_unknown_length() {
    for asynchronous in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let mut store = BasisStore::new(&directory);
        let (url, server) = serve(vec![
            "HTTP/1.1 302 Found\r\nLocation: /redirected\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_owned(),
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n3\r\nabc\r\n2\r\nde\r\n0\r\n\r\n".to_owned()
        ]);
        store.url = url;
        let mut progress = Vec::new();
        if asynchronous {
            block_on(store.download("test", &mut |current, total| {
                progress.push((current, total))
            }))
            .unwrap();
            assert_eq!(progress.last(), Some(&(5, None)));
            assert!(progress.iter().all(|(_, total)| total.is_none()));
        } else {
            store.download_sync("test").unwrap();
        }
        assert_eq!(
            fs::read(directory.path().join("test.json")).unwrap(),
            b"abcde"
        );
        assert!(server.join().unwrap()[1].starts_with("GET /redirected "));
    }
}

#[test]
fn cancelling_async_download_preserves_destination_and_cleans_temporary() {
    use std::{cell::Cell, future::Future, task::Poll};

    let directory = tempfile::tempdir().unwrap();
    let mut store = BasisStore::new(&directory);
    let destination = directory.path().join("test.json");
    fs::write(&destination, "original").unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    store.url = Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
    let (release, wait) = std::sync::mpsc::channel();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut reader = io::BufReader::new(&mut stream);
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if line == "\r\n" || line.is_empty() {
                break;
            }
        }
        stream
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\npartial",
            )
            .unwrap();
        wait.recv_timeout(Duration::from_secs(10)).unwrap();
    });

    let received = Cell::new(false);
    let mut callback = |_, _| received.set(true);
    let mut download = Box::pin(store.download("test", &mut callback));
    block_on(std::future::poll_fn(|cx| {
        assert!(download.as_mut().poll(cx).is_pending());
        if received.get() {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }));
    drop(download);
    release.send(()).unwrap();
    server.join().unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    // An in-flight disk write may finish on the I/O pool after cancellation.
    while fs::read_dir(directory.path()).unwrap().count() != 1 {
        assert!(
            Instant::now() < deadline,
            "temporary file was not cleaned up"
        );
        thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(fs::read(destination).unwrap(), b"original");
}
