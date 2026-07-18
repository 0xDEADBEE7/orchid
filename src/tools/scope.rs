use globset::{Glob, GlobSet};
use std::env;
use std::path::Path;

pub fn expand_path(s: &str, working_dir: &str) -> String {
    if s.starts_with("~") {
        let home = env::var("HOME")
            .ok()
            .or_else(|| dirs::home_dir().map(|p| p.to_string_lossy().to_string()));

        if let Some(home_str) = home {
            if s == "~" {
                home_str
            } else if let Some(rest) = s.strip_prefix("~/") {
                format!("{}/{}", home_str, rest)
            } else {
                s.to_string()
            }
        } else {
            s.to_string()
        }
    } else if s.contains("${") || s.contains("$") {
        expand_vars(s)
    } else if s.starts_with("/") {
        s.to_string()
    } else {
        format!("{}/{}", working_dir, s)
    }
}

fn expand_vars(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '$' {
            if chars.peek() == Some(&'{') {
                chars.next();
                let mut var_name = String::new();
                for c in chars.by_ref() {
                    if c == '}' {
                        break;
                    }
                    var_name.push(c);
                }
                match env::var(&var_name) {
                    Ok(val) => result.push_str(&val),
                    Err(_) => result.push_str(&format!("${{{}}}", var_name)),
                }
            } else {
                let mut var_name = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_alphanumeric() || c == '_' {
                        var_name.push(c);
                        chars.next();
                    } else {
                        break;
                    }
                }
                if !var_name.is_empty() {
                    match env::var(&var_name) {
                        Ok(val) => result.push_str(&val),
                        Err(_) => result.push_str(&format!("${}", var_name)),
                    }
                } else {
                    result.push('$');
                }
            }
        } else {
            result.push(ch);
        }
    }

    result
}

/// Compile a list of glob patterns into a GlobSet.
///
/// Invalid patterns are silently skipped (fallback to empty GlobSet).
/// Patterns starting with `~` are expanded using the HOME env var.
pub fn compile_exceptions(patterns: &[String]) -> GlobSet {
    use globset::GlobSetBuilder;
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let expanded = expand_pattern_tilde(pattern);
        if let Ok(glob) = Glob::new(&expanded) {
            builder.add(glob);
        }
    }
    builder.build().unwrap_or_else(|_| GlobSet::empty())
}

/// Expand `~` at the start of a pattern to the home directory.
fn expand_pattern_tilde(pattern: &str) -> String {
    if let Some(rest) = pattern.strip_prefix("~/") {
        if let Ok(home) = env::var("HOME") {
            return format!("{}/{}", home, rest);
        }
    }
    pattern.to_string()
}

/// Check if a path is allowed through scope enforcement.
///
/// Resolution order:
/// 1. In-scope (path starts with working_dir) → allowed
/// 2. Global exceptions match → allowed
/// 3. Per-conversation exceptions match → allowed
/// 4. Nothing matched → denied
pub fn is_allowed(
    path: &str,
    working_dir: &str,
    global_set: &GlobSet,
    convo_set: &GlobSet,
) -> bool {
    let expanded = expand_path(path, working_dir);

    // 1. In-scope
    if is_in_scope(&expanded, working_dir) {
        return true;
    }

    // 2. Global exceptions
    if global_set.is_match(&expanded) {
        return true;
    }

    // 3. Per-conversation exceptions
    if convo_set.is_match(&expanded) {
        return true;
    }

    false
}

pub fn is_in_scope(path: &str, working_dir: &str) -> bool {
    let expanded = expand_path(path, working_dir);
    let canonical_path = normalize_path(&expanded);
    let canonical_working = normalize_path(working_dir);

    // Guard against mismatches caused by partial symlink resolution (e.g. macOS
    // /tmp -> /private/tmp): if the working dir resolved through a symlink we
    // also accept paths that start with the *unresolved* working dir string.
    canonical_path.starts_with(&canonical_working) || expanded.starts_with(working_dir)
}

fn normalize_path(p: &str) -> String {
    let path = Path::new(p);
    match path.canonicalize() {
        Ok(canonical) => canonical.to_string_lossy().to_string(),
        Err(_) => {
            let mut normalized = String::new();
            for component in path.components() {
                use std::path::Component;
                match component {
                    Component::ParentDir => {
                        if let Some(last_slash) = normalized.rfind('/') {
                            normalized.truncate(last_slash);
                        }
                    }
                    Component::CurDir => {}
                    Component::Normal(os_str) => {
                        if !normalized.is_empty() && !normalized.ends_with('/') {
                            normalized.push('/');
                        }
                        normalized.push_str(&os_str.to_string_lossy());
                    }
                    Component::RootDir => {
                        normalized.push('/');
                    }
                    Component::Prefix(_) => {}
                }
            }
            if normalized.is_empty() {
                "/".to_string()
            } else {
                normalized
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_path_absolute() {
        let result = expand_path("/tmp/test", "/home/user");
        assert_eq!(result, "/tmp/test");
    }

    #[test]
    fn test_expand_path_relative() {
        let result = expand_path("file.txt", "/home/user/work");
        assert_eq!(result, "/home/user/work/file.txt");
    }

    #[test]
    fn test_is_in_scope_tmp_when_working_dir_is_tmp() {
        assert!(is_in_scope("/tmp/test", "/tmp"));
    }

    #[test]
    fn test_is_in_scope_tmp_outside_working_dir() {
        // /tmp is no longer implicitly in scope — must be the working dir itself
        assert!(!is_in_scope("/tmp/any/path", "/home/user"));
    }

    #[test]
    fn test_is_in_scope_var_folders_outside_working_dir() {
        // /var/folders is no longer implicitly in scope
        assert!(!is_in_scope("/var/folders/abc/x", "/home/user"));
    }

    #[test]
    fn test_expand_vars_simple() {
        env::set_var("TEST_VAR", "value");
        let result = expand_vars("prefix_$TEST_VAR");
        assert_eq!(result, "prefix_value");
    }

    #[test]
    fn test_compile_exceptions_empty() {
        let set = compile_exceptions(&Vec::<String>::new());
        assert!(!set.is_match("/any/path"));
    }

    #[test]
    fn test_compile_exceptions_matches() {
        env::set_var("HOME", "/home/user");
        let patterns = vec!["**/target/**".to_string(), "~/.cache/**".to_string()];
        let set = compile_exceptions(&patterns);
        assert!(set.is_match("/home/user/project/target/output.o"));
        assert!(set.is_match("/home/user/.cache/orchid/thing"));
        assert!(!set.is_match("/home/user/project/src/main.rs"));
        env::remove_var("HOME");
    }

    #[test]
    fn test_is_allowed_in_scope() {
        let empty = GlobSet::empty();
        assert!(is_allowed(
            "/home/user/project/file.txt",
            "/home/user/project",
            &empty,
            &empty
        ));
    }

    #[test]
    fn test_is_allowed_global_exception() {
        let patterns = vec!["**/target/**".to_string()];
        let global = compile_exceptions(&patterns);
        let empty = GlobSet::empty();
        assert!(is_allowed(
            "/home/user/project/target/out",
            "/home/user/project",
            &global,
            &empty
        ));
        assert!(!is_allowed(
            "/etc/passwd",
            "/home/user/project",
            &global,
            &empty
        ));
    }

    #[test]
    fn test_is_allowed_convo_exception() {
        let patterns = vec!["**/node_modules/**".to_string()];
        let empty = GlobSet::empty();
        let convo = compile_exceptions(&patterns);
        assert!(is_allowed(
            "/home/user/project/node_modules/pkg/index.js",
            "/home/user/project",
            &empty,
            &convo
        ));
        assert!(!is_allowed(
            "/etc/hosts",
            "/home/user/project",
            &empty,
            &convo
        ));
    }

    #[test]
    fn test_is_allowed_no_match_denied() {
        let empty = GlobSet::empty();
        assert!(!is_allowed(
            "/etc/passwd",
            "/home/user/project",
            &empty,
            &empty
        ));
    }


    #[test]
    fn test_compile_exceptions_tmp_pattern() {
        env::set_var("HOME", "/home/user");
        let patterns = vec!["**/tmp/**".to_string()];
        let set = compile_exceptions(&patterns);
        assert!(set.is_match("/tmp/scope_test_file.txt"));
        assert!(set.is_match("/Users/user/tmp/build/output.o"));
        assert!(!set.is_match("/home/user/project/src/main.rs"));
        env::remove_var("HOME");
    }
}
