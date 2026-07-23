pub fn help() -> Result<serde_json::Value, String> {
    let text = r#"orchid - session management CLI

USAGE:
  orchid <COMMAND> [OPTIONS]

COMMANDS:
  list                List sessions or resources
  config              Validate/list/show resources
  create              Create a new session without sending a message
  send                Send message to session (requires --id or stores in current)
  await               Wait for one or more sessions to reach a terminal state
  get                 Read conversation, metadata, or state
  set                 Update session settings
  delete              Delete session by ID
  stop                Stop a running session (alias for kill)
  kill                Kill a running session (alias for stop)
  server-action       Execute a server action (list/load/unload models)
  help                Display this help message

OPTIONS:
  --config <DIR>      Use a config directory (required for new config model)
  --conversation      Read the session conversation
  --last-message      Read the latest assistant message
  --metadata          Read session metadata
  --state             Read session state
  --help              Show help for a command
  --id <ID>           Session ID
  --await             Wait for completion after send
  --label <TEXT>      Set session label
  --working-dir <PATH> Set working directory

EXAMPLES:
  orchid help                              Show this help
  orchid list                              List sessions
  orchid create --label "my-task" --working-dir /path/to/project
  orchid send "hello" --id abc123          Send message
  config validate --config ./config    Validate config directory
  config list --config ./config        List resources
  config show policy/default            Inspect a policy
  orchid set --id abc123 --label "work"    Update label
  orchid server-action list_models         Execute server action

For command-specific help: orchid <COMMAND> --help"#;

    println!("{}", text);
    Ok(serde_json::Value::Null)
}

pub fn help_command(cmd: &str) -> Result<serde_json::Value, String> {
    let text = match cmd {
        "list" => "orchid list - List all sessions\n\nUsage: orchid list\n\nShows all stored sessions.",
        "get" => "orchid get - Read session resources\n\nUsage: orchid get <SESSION_ID> [--conversation] [--last-message] [--metadata] [--state]",
        "config" => "orchid config - Inspect configuration resources\n\nUsage: orchid config <SUBCOMMAND> [--config <DIR>]\n\nSubcommands:\n  validate         Validate the selected resource directory\n  list             List connections, policies, prompts, and auth\n  show <RESOURCE>  Inspect root, connection/name, policy/name, prompt/name, or auth/name",
        "auth" => "orchid auth - Inspect authentication profiles\n\nUsage: orchid auth list|validate <name> [--config <DIR>]",
        "create" => "orchid create - Create a new session\n\nUsage: orchid create [OPTIONS]\n\nOptions:\n  --label <TEXT>       Set display name\n  --working-dir <PATH> Set working directory",
        "send" => "orchid send - Send message to session\n\nUsage: orchid send <MESSAGE> [OPTIONS]\n\nOptions:\n  --config <DIR>     Use config directory (required)\n  --id <ID>          Target session (required if no current)\n  --await            Wait for response\n  --label <TEXT>     Set session label",
        "await" => "orchid await - Wait for sessions to finish\n\nUsage: orchid await <SESSION_ID>... [OPTIONS]\n\nOptions:\n  --timeout <SECONDS>  Overall deadline (default: 60)\n  --interval <SECONDS> Poll interval (default: 2)\n\nReports idle, failed, and cancelled sessions as completed. A timeout exits with code 2. The command observes sessions; it does not control or cancel them.",
        "set" => "orchid set - Update session settings\n\nUsage: orchid set --id <ID> [OPTIONS]\n\nOptions:\n  --label <TEXT>       Set display name\n  --working-dir <PATH> Set working directory",

        "delete" => "orchid delete - Archive session\n\nUsage: orchid delete <ID>\n\nMoves the session to ~/.config/orchid/sessions/.archive/<id>.\nRemoved from orchid list. Reversible: move the directory back to restore.",
        "stop" => "orchid stop - Stop a running session\n\nUsage: orchid stop <ID>\n\nSends SIGTERM to the session's background process, then marks it as Idle.\n\nAlias: kill",
        "kill" => "orchid kill - Kill a running session\n\nUsage: orchid kill <ID>\n\nSends SIGKILL to the session's background process, then marks it as Idle.\n\nAlias: stop.",
        "server-action" => "orchid server-action - Execute a server action\n\nThis command is not available in the resource-oriented configuration model. Use a configured Connection through create/send instead.",
        "help" => "orchid help - Display help\n\nUsage: orchid help\n       orchid --help\n       orchid <COMMAND> --help\n\nShow usage information.",
        _ => return Err(format!("unknown command: {}", cmd)),
    };

    println!("{}", text);
    Ok(serde_json::Value::Null)
}
