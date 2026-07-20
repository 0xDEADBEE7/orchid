pub fn help() -> Result<serde_json::Value, String> {
    let text = r#"orchid - conversation management CLI

USAGE:
  orchid <COMMAND> [OPTIONS]

COMMANDS:
  list                List sessions or resources
  config              Validate/list/show resources
  create              Create a new conversation without sending a message
  send                Send message to conversation (requires --id or stores in current)
  set                 Update conversation settings
  delete              Delete conversation by ID
  stop                Stop a running conversation (alias for kill)
  kill                Kill a running conversation (alias for stop)
  server-action       Execute a server action (list/load/unload models)
  help                Display this help message

OPTIONS:
  --config <DIR>      Use a config directory (required for new config model)
  --help              Show help for a command
  --id <ID>           Conversation ID
  --await             Wait for completion after send
  --label <TEXT>      Set conversation label
  --working-dir <PATH> Set working directory

EXAMPLES:
  orchid help                              Show this help
  orchid list                              List conversations
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
        "list" => "orchid list - List all conversations\n\nUsage: orchid list\n\nShows all stored conversations.",
        "config" => "orchid config - Inspect configuration resources\n\nUsage: orchid config <SUBCOMMAND> [--config <DIR>]\n\nSubcommands:\n  validate         Validate the selected resource directory\n  list             List connections, policies, and prompts\n  show <RESOURCE>  Inspect root, connection/name, policy/name, or prompt/name",
        "create" => "orchid create - Create a new conversation\n\nUsage: orchid create [OPTIONS]\n\nOptions:\n  --label <TEXT>       Set display name\n  --working-dir <PATH> Set working directory",
        "send" => "orchid send - Send message to conversation\n\nUsage: orchid send <MESSAGE> [OPTIONS]\n\nOptions:\n  --config <DIR>     Use config directory (required)\n  --id <ID>          Target conversation (required if no current)\n  --await            Wait for response\n  --label <TEXT>     Set conversation label",
        "set" => "orchid set - Update session settings\n\nUsage: orchid set --id <ID> [OPTIONS]\n\nOptions:\n  --label <TEXT>       Set display name\n  --working-dir <PATH> Set working directory",

        "delete" => "orchid delete - Archive conversation\n\nUsage: orchid delete <ID>\n\nMoves the conversation to ~/.config/orchid/conversations/.archive/<id>.\nRemoved from orchid list. Reversible: move the directory back to restore.",
        "stop" => "orchid stop - Stop a running conversation\n\nUsage: orchid stop <ID>\n\nSends SIGTERM to the conversation's background process, then marks it as Idle.\n\nAlias: kill",
        "kill" => "orchid kill - Kill a running conversation\n\nUsage: orchid kill <ID>\n\nSends SIGKILL to the conversation's background process, then marks it as Idle.\n\nAlias: stop.",
        "server-action" => "orchid server-action - Execute a server action\n\nThis command is not available in the resource-oriented configuration model. Use a configured Connection through create/send instead.",
        "help" => "orchid help - Display help\n\nUsage: orchid help\n       orchid --help\n       orchid <COMMAND> --help\n\nShow usage information.",
        _ => return Err(format!("unknown command: {}", cmd)),
    };

    println!("{}", text);
    Ok(serde_json::Value::Null)
}
