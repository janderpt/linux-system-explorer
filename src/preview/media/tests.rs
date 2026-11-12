use super::*;

fn runtime() -> Option<&'static Api> {
    match Api::get() {
        Ok(api) => Some(api),
        Err(error) => {
            assert!(
                std::env::var_os("CI").is_none(),
                "Missing media runtime in CI: {error}"
            );
            eprintln!("Skipping media integration without GStreamer: {error}");
            None
        }
    }
}

fn wave(path: &std::path::Path) {
    let length = 8000_u32;
    let mut data = Vec::new();
    data.extend(b"RIFF");
    data.extend((36 + length).to_le_bytes());
    data.extend(b"WAVEfmt ");
    data.extend(16_u32.to_le_bytes());
    data.extend(1_u16.to_le_bytes());
    data.extend(1_u16.to_le_bytes());
    data.extend(8000_u32.to_le_bytes());
    data.extend(16000_u32.to_le_bytes());
    data.extend(2_u16.to_le_bytes());
    data.extend(16_u16.to_le_bytes());
    data.extend(b"data");
    data.extend(length.to_le_bytes());
    data.resize(44 + length as usize, 0);
    std::fs::write(path, data).unwrap();
}
#[test]
fn audio_decodes_reaches_end_replays_and_releases_its_worker_without_an_audio_device() {
    if runtime().is_none() {
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("silence.wav");
    wave(&path);
    let ctx = eframe::egui::Context::default();
    let session = Session::start_with_audio(path, &ctx, true);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let state = session.poll();
        assert!(state.error.is_none(), "{:?}", state.error);
        if state.ended {
            assert!(state.duration > 0.4);
            break;
        }
        assert!(Instant::now() < deadline, "EOS not received");
        std::thread::sleep(Duration::from_millis(10));
    }
    session.control(Control::Playing(true));
    loop {
        let state = session.poll();
        assert!(state.error.is_none(), "{:?}", state.error);
        if state.playing && !state.ended {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
    session.control(Control::Playing(false));
    loop {
        if !session.poll().playing {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
    session.control(Control::Seek(0.2));
    session.control(Control::Volume(0.0));
    let weak = Arc::downgrade(&session.state);
    drop(session);
    while weak.upgrade().is_some() {
        assert!(Instant::now() < deadline, "media worker not released");
        std::thread::sleep(Duration::from_millis(10));
    }
}
#[test]
fn video_frames_are_decoded_inside_the_application_and_corrupt_media_reports_an_error() {
    if runtime().is_none() {
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("video.webm");
    let status = std::process::Command::new("ffmpeg")
        .args([
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=c=red:s=64x48:r=5",
            "-t",
            "0.6",
            "-c:v",
            "libvpx",
            "-an",
            "-y",
        ])
        .arg(&path)
        .status();
    if status.is_err() {
        assert!(std::env::var_os("CI").is_none(), "ffmpeg is required in CI");
        return;
    }
    assert!(status.unwrap().success());
    let ctx = eframe::egui::Context::default();
    let session = Session::start_with_audio(path, &ctx, true);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let state = session.poll();
        assert!(state.error.is_none(), "{:?}", state.error);
        if let Some(frame) = state.frame {
            assert_eq!(
                (frame.width, frame.height, frame.rgba.len()),
                (960, 540, 960 * 540 * 4)
            );
            let center = (270 * 960 + 480) * 4;
            assert!(frame.rgba[center] > 180 && frame.rgba[center + 1] < 80);
            break;
        }
        assert!(Instant::now() < deadline, "no video frame");
        std::thread::sleep(Duration::from_millis(10));
    }
    drop(session);
    let bad = root.path().join("bad.mp4");
    std::fs::write(&bad, b"not a movie").unwrap();
    let session = Session::start_with_audio(bad, &ctx, true);
    loop {
        let state = session.poll();
        if state.error.is_some() {
            assert!(!state.playing);
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn mp4_h264_with_audio_plays_to_completion_when_the_declared_codec_is_installed() {
    let Some(api) = runtime() else {
        return;
    };
    unsafe {
        let codec = (api.make)(c"avdec_h264".as_ptr(), std::ptr::null());
        if codec.is_null() {
            assert!(
                std::env::var_os("CI").is_none(),
                "Missing declared H.264 codec in CI"
            );
            eprintln!("MP4 integration requires the declared gstreamer1.0-libav runtime package");
            return;
        }
        (api.unref)(codec);
    }
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("movie.mp4");
    let result = std::process::Command::new("ffmpeg")
        .args([
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=c=blue:s=128x72:r=10",
            "-f",
            "lavfi",
            "-i",
            "anullsrc=r=44100:cl=mono",
            "-t",
            "0.6",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-y",
        ])
        .arg(&path)
        .status();
    if result.is_err() {
        assert!(std::env::var_os("CI").is_none(), "ffmpeg is required in CI");
        return;
    }
    assert!(result.unwrap().success());
    let ctx = eframe::egui::Context::default();
    let session = Session::start_with_audio(path, &ctx, true);
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut decoded = false;
    loop {
        let state = session.poll();
        assert!(state.error.is_none(), "{:?}", state.error);
        decoded |= state.frame.is_some();
        if state.ended {
            assert!(decoded);
            assert!(state.duration > 0.4);
            break;
        }
        assert!(Instant::now() < deadline, "MP4 did not finish");
        std::thread::sleep(Duration::from_millis(10));
    }
}
