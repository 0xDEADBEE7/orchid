use orchid::cli::{parse_args, Command, ConfigSubcommand};

mod support;

#[test]
fn test_parse_list() {
    let args = vec!["send".to_string(), "list".to_string()];
    let (cmd, flags) = parse_args(&args).unwrap();
    assert_eq!(cmd, Command::List(None));
    assert!(flags.is_empty());
}

#[test]
fn test_parse_config_current() {
    let args = vec![
        "send".to_string(),
        "config".to_string(),
        "current".to_string(),
    ];
    let (cmd, _) = parse_args(&args).unwrap();
    assert_eq!(cmd, Command::Config(ConfigSubcommand::Current));
}

#[test]
fn test_parse_config_use() {
    let args = vec![
        "send".to_string(),
        "config".to_string(),
        "use".to_string(),
        "myprofile".to_string(),
    ];
    let (cmd, _) = parse_args(&args).unwrap();
    assert_eq!(
        cmd,
        Command::Config(ConfigSubcommand::Use("myprofile".to_string()))
    );
}

#[test]
fn test_parse_config_path() {
    let args = vec!["send".to_string(), "config".to_string(), "path".to_string()];
    let (cmd, _) = parse_args(&args).unwrap();
    assert_eq!(cmd, Command::Config(ConfigSubcommand::Path));
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
fn test_parse_config_use_no_profile() {
    let args = vec!["send".to_string(), "config".to_string(), "use".to_string()];
    assert!(parse_args(&args).is_err());
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
        "--profile".to_string(),
        "myprofile".to_string(),
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
fn test_parse_server_action_minimal() {
    let args = vec![
        "send".to_string(),
        "server-action".to_string(),
        "list_models".to_string(),
    ];
    let (cmd, _) = parse_args(&args).unwrap();
    match cmd {
        Command::ServerAction {
            action,
            profile,
            body_params,
        } => {
            assert_eq!(action, "list_models");
            assert!(profile.is_none());
            assert!(body_params.is_empty());
        }
        _ => panic!("expected ServerAction"),
    }
}

#[test]
fn test_parse_server_action_with_profile() {
    let args = vec![
        "send".to_string(),
        "server-action".to_string(),
        "load_model".to_string(),
        "--profile".to_string(),
        "local-lmstudio".to_string(),
    ];
    let (cmd, _) = parse_args(&args).unwrap();
    match cmd {
        Command::ServerAction {
            action,
            profile,
            body_params,
        } => {
            assert_eq!(action, "load_model");
            assert_eq!(profile, Some("local-lmstudio".to_string()));
            assert!(body_params.is_empty());
        }
        _ => panic!("expected ServerAction"),
    }
}

#[test]
fn test_parse_server_action_with_body_params() {
    let args = vec![
        "send".to_string(),
        "server-action".to_string(),
        "load_model".to_string(),
        "--profile".to_string(),
        "local-lmstudio".to_string(),
        "--model".to_string(),
        "openai/gpt-oss-20b".to_string(),
        "--context_length".to_string(),
        "16384".to_string(),
    ];
    let (cmd, _) = parse_args(&args).unwrap();
    match cmd {
        Command::ServerAction {
            action,
            profile,
            body_params,
        } => {
            assert_eq!(action, "load_model");
            assert_eq!(profile, Some("local-lmstudio".to_string()));
            assert_eq!(body_params.len(), 2);
            assert_eq!(
                body_params[0],
                ("context_length".to_string(), "16384".to_string())
            );
            assert_eq!(
                body_params[1],
                ("model".to_string(), "openai/gpt-oss-20b".to_string())
            );
        }
        _ => panic!("expected ServerAction"),
    }
}

#[test]
fn test_parse_server_action_missing_action() {
    let args = vec!["send".to_string(), "server-action".to_string()];
    let err = parse_args(&args).unwrap_err();
    assert!(err.contains("requires <action>"));
}

#[test]
fn test_parse_server_action_with_eq_flag() {
    let args = vec![
        "send".to_string(),
        "server-action".to_string(),
        "load_model".to_string(),
        "--model=openai/gpt-oss-20b".to_string(),
    ];
    let (cmd, _) = parse_args(&args).unwrap();
    match cmd {
        Command::ServerAction { body_params, .. } => {
            assert_eq!(body_params.len(), 1);
            assert_eq!(
                body_params[0],
                ("model".to_string(), "openai/gpt-oss-20b".to_string())
            );
        }
        _ => panic!("expected ServerAction"),
    }
}
