use super::*;

fn request() -> OpenRequest {
    OpenRequest {
        folder: "/tmp/a".into(),
        selected: vec!["/tmp/a/file".into()],
        properties: false,
    }
}

#[test]
fn closed_timed_out_or_mismatched_windows_cannot_claim_requests() {
    let (queued, reply) = Request::for_test(request());
    assert!(queued.accept(Path::new("/tmp/another")).is_none());
    assert!(!reply.recv().unwrap());
    let (mut queued, reply) = Request::for_test(request());
    queued.deadline = Instant::now() - Duration::from_secs(1);
    assert!(queued.accept(Path::new("/tmp/a")).is_none());
    assert!(!reply.recv().unwrap());
    let (queued, reply) = Request::for_test(request());
    drop(reply);
    assert!(queued.accept(Path::new("/tmp/a")).is_none());
    let (queued, reply) = Request::for_test(request());
    let (accepted, token) = queued.accept(Path::new("/tmp/a")).unwrap();
    assert!(reply.recv().unwrap());
    assert_eq!(accepted, request());
    assert_eq!(token, "test-token");
}
