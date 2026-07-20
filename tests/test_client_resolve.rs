use orchid::resolve_env_inline_strict;

mod support;

#[test]
fn test_resolve_env_inline_whole_value() {
    std::env::set_var("TEST_INLINE_VAR", "mytoken");
    assert_eq!(
        resolve_env_inline_strict("env.TEST_INLINE_VAR").unwrap(),
        "mytoken"
    );
}

#[test]
fn test_resolve_env_inline_with_prefix() {
    std::env::set_var("TEST_INLINE_VAR", "mytoken");
    assert_eq!(
        resolve_env_inline_strict("Bearer env.TEST_INLINE_VAR").unwrap(),
        "Bearer mytoken"
    );
}

#[test]
fn test_resolve_env_inline_unset_var() {
    std::env::remove_var("TEST_INLINE_MISSING");
    let error = resolve_env_inline_strict("Bearer env.TEST_INLINE_MISSING").unwrap_err();
    assert_eq!(error.variables, vec!["TEST_INLINE_MISSING"]);
}
