use super::*;
use std::{io::Cursor, sync::atomic::Ordering};

struct Stream {
    data: Cursor<Vec<u8>>,
    cancel: Option<Arc<AtomicBool>>,
    seek_error: i32,
}
impl Stream {
    fn new(data: Vec<u8>) -> Self {
        Self {
            data: Cursor::new(data),
            cancel: None,
            seek_error: rustix::io::Errno::SPIPE.raw_os_error(),
        }
    }
}
impl Seek for Stream {
    fn seek(&mut self, _: SeekFrom) -> io::Result<u64> {
        Err(io::Error::from_raw_os_error(self.seek_error))
    }
}
impl Read for Stream {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if let Some(cancel) = &self.cancel {
            cancel.store(true, Ordering::Release);
        }
        self.data.read(buffer)
    }
}

#[test]
fn streaming_pdf_gets_a_private_seekable_snapshot_that_renders_and_is_removed_after_use() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("source.pdf");
    crate::preview::tests::test_pdf(&path);
    let bytes = std::fs::read(&path).unwrap();
    let cancel = AtomicBool::new(false);
    let document = prepare_reader(
        Stream::new(bytes.clone()),
        bytes.len() as u64,
        MAX_SNAPSHOT_BYTES,
        &cancel,
    )
    .unwrap()
    .unwrap();
    let cached = document.path();
    assert_eq!(std::fs::read(&cached).unwrap(), bytes);
    let clone = document.clone();
    for page in [1, 2] {
        let super::super::Content::Pdf {
            page: rendered,
            pages,
            document: nested,
            ..
        } = super::super::pdf(&cached, page, &cancel).unwrap()
        else {
            panic!("PDF content")
        };
        assert_eq!((rendered, pages), (page, 2));
        assert!(nested.is_none(), "local cached pages are not copied again");
    }
    drop(document);
    assert!(cached.exists(), "a page worker can retain the cache");
    drop(clone);
    assert!(!cached.exists());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert!(
        prepare(&path, &cancel).unwrap().is_none(),
        "normal local PDFs stay direct"
    );
}

#[test]
fn stream_snapshot_propagates_errors_rejects_partial_reads_and_honors_limits_and_cancellation() {
    let cancel = Arc::new(AtomicBool::new(false));
    assert!(prepare_reader(Stream::new(vec![1; 16]), 16, 8, &cancel).is_err());
    assert!(
        prepare_reader(Stream::new(vec![1; 16]), 0, 8, &cancel).is_err(),
        "enforce actual bytes too"
    );
    assert_eq!(
        prepare_reader(Stream::new(vec![1; 4]), 8, 32, &cancel)
            .unwrap_err()
            .kind(),
        io::ErrorKind::InvalidData
    );
    let mut stream = Stream::new(vec![1; 8]);
    stream.seek_error = rustix::io::Errno::ACCESS.raw_os_error();
    assert_eq!(
        prepare_reader(stream, 8, 32, &cancel).unwrap_err().kind(),
        io::ErrorKind::PermissionDenied
    );
    let mut stream = Stream::new(vec![1; 8]);
    stream.cancel = Some(cancel.clone());
    assert_eq!(
        prepare_reader(stream, 8, 32, &cancel).unwrap_err().kind(),
        io::ErrorKind::Interrupted
    );
    assert_eq!(
        prepare_reader(Stream::new(vec![]), 0, 32, &cancel)
            .unwrap_err()
            .kind(),
        io::ErrorKind::Interrupted
    );
    cancel.store(false, Ordering::Release);
    let root = tempfile::tempdir().unwrap();
    let fifo = root.path().join("pipe.pdf");
    rustix::fs::mknodat(
        rustix::fs::CWD,
        &fifo,
        rustix::fs::FileType::Fifo,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
        0,
    )
    .unwrap();
    assert!(
        prepare(&fifo, &cancel).is_err(),
        "never wait for a special-file writer"
    );
}

#[test]
#[ignore = "Read-only remote PDF integration; requires LSE_TEST_PREVIEW_PDF (an existing mounted PDF)"]
fn mounted_streaming_pdf_renders_pages_thumbnails_and_print_preparation_without_changing_source() {
    let path = std::env::var_os("LSE_TEST_PREVIEW_PDF")
        .map(std::path::PathBuf::from)
        .expect("set a mounted PDF path");
    let cancel = AtomicBool::new(false);
    let original = std::fs::read(&path).unwrap();
    let super::super::Content::Pdf {
        document, pages, ..
    } = super::super::pdf(&path, 1, &cancel).unwrap()
    else {
        panic!("PDF content")
    };
    let document = document.expect("non-seekable mount requires a snapshot");
    let cached = document.path();
    assert_eq!(std::fs::read(&cached).unwrap(), original);
    assert!(pages > 0);
    super::super::pdf(&cached, pages, &cancel).unwrap();
    let ctx = eframe::egui::Context::default();
    let job =
        super::super::thumbnail::start(path.clone(), path.clone(), super::super::Kind::Pdf, &ctx)
            .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let pixels = loop {
        if let Some(result) = job.poll() {
            let super::super::Content::Image(pixels) = result.unwrap() else {
                panic!("thumbnail")
            };
            break pixels;
        }
        assert!(std::time::Instant::now() < deadline, "thumbnail timed out");
        std::thread::sleep(std::time::Duration::from_millis(5));
    };
    assert!(pixels.width > 0 && pixels.height > 0);
    let entry = crate::filesystem::read_directory(path.parent().unwrap())
        .unwrap()
        .entries
        .into_iter()
        .find(|entry| entry.path == path)
        .unwrap();
    let prepared = super::super::print::prepare(&entry, &cancel).unwrap();
    assert_ne!(prepared.path, path);
    super::super::pdf(&prepared.path, 1, &cancel).unwrap();
    let printed = prepared.path.clone();
    drop(prepared);
    assert!(!printed.exists());
    drop(document);
    assert!(!cached.exists());
    assert_eq!(std::fs::read(path).unwrap(), original);
}
