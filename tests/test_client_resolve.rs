use orchid::resolve_env_inline;

mod support;

#[test]
fn test_resolve_env_inline_whole_value() {
    std::env::set_var("TEST_INLINE_VAR", "mytoken");
    assert_eq!(resolve_env_inline("env.TEST_INLINE_VAR"), "mytoken");
}

#[test]
fn test_resolve_env_inline_with_prefix() {
    std::env::set_var("TEST_INLINE_VAR", "mytoken");
    assert_eq!(
        resolve_env_inline("Bearer env.TEST_INLINE_VAR"),
        "Bearer mytoken"
    );
}

#[test]
fn test_resolve_env_inline_unset_var() {
    std::env::remove_var("TEST_INLINE_MISSING");
    assert_eq!(
        resolve_env_inline("Bearer env.TEST_INLINE_MISSING"),
        "Bearer "
    );
}
