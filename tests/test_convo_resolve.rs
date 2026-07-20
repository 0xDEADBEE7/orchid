use orchid::{is_id_format, resolve};
use tempfile::TempDir;

mod support;

#[test]
fn test_is_id_format() {
    assert!(is_id_format("abcdef0123456789abcdef0123456789"));
    assert!(!is_id_format("short"));
    assert!(!is_id_format("not_hex_!@#$%abcdef0123456789abcdef01234"));
}

#[test]
fn test_resolve_rejects_non_id() {
    let temp = TempDir::new().unwrap();
    let err = resolve("my-label", temp.path()).unwrap_err();
    assert!(err.contains("invalid session ID"), "got: {}", err);
}
