use orchid::generate_id;

mod support;

#[test]
fn test_generate_id_format() {
    let id = generate_id();
    assert_eq!(id.len(), 32);
    assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn test_generate_id_unique() {
    let id1 = generate_id();
    let id2 = generate_id();
    assert_ne!(id1, id2);
}
