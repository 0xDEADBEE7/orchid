//! Command-line argument helpers shared by the CLI commands.
use std::io;

/// Returns positional arguments, skipping flags and their values; everything
/// after `--` is positional.
pub fn positional(args: &[String]) -> Vec<String> {
    let flags = [
        "--id",
        "--label",
        "--working-dir",
        "--agent",
        "--timeout",
        "--interval",
    ];
    let mut result = Vec::new();
    let mut skip = false;
    for (index, arg) in args.iter().enumerate() {
        if skip {
            skip = false;
            continue;
        }
        if arg == "--" {
            // Everything after `--` is literal, e.g. a message starting with '-'.
            result.extend(args[index + 1..].iter().cloned());
            break;
        }
        if flags.contains(&arg.as_str()) {
            skip = true;
            continue;
        }
        if !arg.starts_with('-') {
            result.push(arg.clone());
        }
    }
    result
}

/// Returns the named argument value, if present.
pub fn value(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|w| w[0] == name)
        .map(|w| w[1].clone())
        .or_else(|| {
            args.iter()
                .find_map(|x| x.strip_prefix(&format!("{name}=")).map(str::to_string))
        })
}

/// Creates an invalid-input error.
pub fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
