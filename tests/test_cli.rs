use orchid::cli::{parse_args, Command, ConfigSubcommand};

mod support;

#[test]
fn test_parse_create_policy() {
    let args = vec![
        "create".to_string(),
        "--policy".to_string(),
        "advanced".to_string(),
    ];
    let (cmd, _) = parse_args(&args).unwrap();
    match cmd {
        Command::Create { policy, .. } => assert_eq!(policy.as_deref(), Some("advanced")),
        _ => panic!("expected Create command"),
    }
}

#[test]
fn test_parse_send_policy() {
    let args = vec![
        "send".to_string(),
        "hello".to_string(),
        "--policy".to_string(),
        "advanced".to_string(),
    ];
    let (cmd, _) = parse_args(&args).unwrap();
    match cmd {
        Command::Send { policy, .. } => assert_eq!(policy.as_deref(), Some("advanced")),
        _ => panic!("expected Send command"),
    }
}
#[test]
fn test_parse_list() {
    let args = vec!["send".to_string(), "list".to_string()];
    let (cmd, flags) = parse_args(&args).unwrap();
    assert_eq!(cmd, Command::List(None));
    assert!(flags.is_empty());
}

#[test]
fn test_parse_config_validate() {
    let args = vec!["config".to_string(), "validate".to_string()];
    let (cmd, _) = parse_args(&args).unwrap();
    assert_eq!(cmd, Command::Config(ConfigSubcommand::Validate));
}

#[test]
fn test_parse_config_list() {
    let args = vec!["config".to_string(), "list".to_string()];
    let (cmd, _) = parse_args(&args).unwrap();
    assert_eq!(cmd, Command::Config(ConfigSubcommand::List));
}

#[test]
fn test_parse_config_show() {
    let args = vec![
        "config".to_string(),
        "show".to_string(),
        "policy/default".to_string(),
    ];
    let (cmd, _) = parse_args(&args).unwrap();
    assert_eq!(
        cmd,
        Command::Config(ConfigSubcommand::Show("policy/default".to_string()))
    );
}

#[test]
fn test_parse_flags() {
    let args = vec![
        "send".to_string(),
        "send".to_string(),
        "--id".to_string(),
        "abc".to_string(),
        "--await".to_string(),
        "the message".to_string(),
    ];
    let (cmd, _) = parse_args(&args).unwrap();
    match cmd {
        Command::Send {
            id,
            await_completion,
            message,
            ..
        } => {
            assert_eq!(id, Some("abc".to_string()));
            assert!(await_completion);
            assert_eq!(message, "the message");
        }
        _ => panic!("expected Send"),
    }
}

#[test]
fn test_parse_no_args() {
    let args: Vec<String> = vec![];
    let (cmd, _) = parse_args(&args).unwrap();
    assert_eq!(cmd, Command::Help(None));
}

#[test]
fn test_parse_config_no_subcommand() {
    let args = vec!["send".to_string(), "config".to_string()];
    assert!(parse_args(&args).is_err());
}

#[test]
fn test_parse_config_legacy_commands_rejected() {
    for args in [
        vec!["config".to_string(), "current".to_string()],
        vec!["config".to_string(), "path".to_string()],
        vec![
            "config".to_string(),
            "use".to_string(),
            "default".to_string(),
        ],
    ] {
        assert!(parse_args(&args).is_err());
    }
}

#[test]
fn test_parse_send_unknown_as_message() {
    // Unknown words after "send" are treated as messages, not commands.
    let args = vec!["send".to_string(), "unknown".to_string()];
    let (cmd, _) = parse_args(&args).unwrap();
    match cmd {
        Command::Send { message, .. } => {
            assert_eq!(message, "unknown");
        }
        _ => panic!("expected Send"),
    }
}

#[test]
fn test_parse_help_command() {
    let args = vec!["send".to_string(), "help".to_string()];
    let (cmd, _) = parse_args(&args).unwrap();
    assert_eq!(cmd, Command::Help(None));
}

#[test]
fn test_parse_help_flag() {
    let args = vec!["send".to_string(), "--help".to_string()];
    let (cmd, _) = parse_args(&args).unwrap();
    assert_eq!(cmd, Command::Help(None));
}

#[test]
fn test_parse_command_help_flag() {
    let args = vec!["send".to_string(), "list".to_string(), "--help".to_string()];
    let (cmd, _) = parse_args(&args).unwrap();
    assert_eq!(cmd, Command::Help(Some("list".to_string())));
}

#[test]
fn test_parse_send() {
    let args = vec![
        "send".to_string(),
        "send".to_string(),
        "hello world".to_string(),
    ];
    let (cmd, _) = parse_args(&args).unwrap();
    match cmd {
        Command::Send {
            id: None,
            message,
            await_completion: false,
            ..
        } => assert_eq!(message, "hello world"),
        _ => panic!("expected Send command"),
    }
}

#[test]
fn test_parse_send_await_does_not_consume_message() {
    let args = vec![
        "send".to_string(),
        "send".to_string(),
        "--id".to_string(),
        "abc123".to_string(),
        "--policy".to_string(),
        "default".to_string(),
        "--await".to_string(),
        "the message".to_string(),
    ];
    let (cmd, _) = parse_args(&args).unwrap();
    match cmd {
        Command::Send {
            message,
            await_completion,
            id,
            ..
        } => {
            assert_eq!(message, "the message");
            assert!(await_completion, "--await should be set");
            assert_eq!(id, Some("abc123".to_string()));
        }
        _ => panic!("expected Send command"),
    }
}

#[test]
fn test_parse_send_with_id() {
    let args = vec![
        "send".to_string(),
        "send".to_string(),
        "--id".to_string(),
        "abc123".to_string(),
        "test message".to_string(),
    ];
    let (cmd, _) = parse_args(&args).unwrap();
    match cmd {
        Command::Send { id: Some(id), .. } => assert_eq!(id, "abc123"),
        _ => panic!("expected Send command with id"),
    }
}

#[test]
fn test_parse_delete() {
    let args = vec![
        "send".to_string(),
        "delete".to_string(),
        "abc123".to_string(),
    ];
    let (cmd, _) = parse_args(&args).unwrap();
    match cmd {
        Command::Delete(id) => assert_eq!(id, "abc123"),
        _ => panic!("expected Delete command"),
    }
}

#[test]
fn test_unknown_flag_is_error() {
    let args = vec![
        "send".to_string(),
        "--unknown".to_string(),
        "value".to_string(),
        "hello".to_string(),
    ];
    let err = parse_args(&args).unwrap_err();
    assert!(err.contains("unknown flag"));
}

#[test]
fn test_unknown_flag_does_not_consume_message() {
    let args = vec![
        "send".to_string(),
        "--unknown".to_string(),
        "hello".to_string(),
        "a message".to_string(),
    ];
    let err = parse_args(&args).unwrap_err();
    assert!(err.contains("unknown flag"));
}

#[test]
fn test_parse_server_action_is_removed() {
    let args = vec!["server-action".to_string(), "list_models".to_string()];
    assert!(parse_args(&args).is_err());
}
