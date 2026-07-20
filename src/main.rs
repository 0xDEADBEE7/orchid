use orchid::cli::{output, parse_args, Command, ConfigSubcommand};
use orchid::cmd;
use orchid::JsonError;
use std::env;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();
    let args_slice = if args.len() > 1 { &args[1..] } else { &[] };

    let mut config_dir =
        orchid::get_orchid_dir().unwrap_or_else(|_| std::path::PathBuf::from("config"));
    let mut filtered_args = Vec::new();
    let mut i = 0;
    while i < args_slice.len() {
        if args_slice[i] == "--config" {
            if i + 1 >= args_slice.len() {
                let err = JsonError::new("invalid_args", "--config requires <directory>");
                let _ = output::print_error(&err);
                process::exit(1);
            }
            config_dir = std::path::PathBuf::from(&args_slice[i + 1]);
            i += 2;
        } else if let Some(path) = args_slice[i].strip_prefix("--config=") {
            config_dir = std::path::PathBuf::from(path);
            i += 1;
        } else {
            filtered_args.push(args_slice[i].clone());
            i += 1;
        }
    }

    let (cmd, _flags) = match parse_args(&filtered_args) {
        Ok((c, f)) => (c, f),
        Err(e) => {
            let err = JsonError::new("invalid_args", &e);
            let _ = output::print_error(&err);
            process::exit(1);
        }
    };

    let result = match cmd {
        Command::Help(None) => cmd::help(),
        Command::Help(Some(ref cmd_name)) => cmd::help_command(cmd_name),
        Command::List(resource) => cmd::list(&config_dir, resource.as_deref()),
        Command::Config(ConfigSubcommand::Validate) => cmd::config_validate(&config_dir),
        Command::Config(ConfigSubcommand::List) => cmd::config_list(&config_dir),
        Command::Config(ConfigSubcommand::Show(resource)) => {
            cmd::config_show(&config_dir, &resource)
        }
        Command::Create {
            label,
            working_dir,
            policy,
            scope_exceptions,
        } => cmd::create(label, working_dir, scope_exceptions, policy, &config_dir),
        Command::Send {
            id,
            message,
            await_completion,
            label,
            working_dir,
            policy,
        } => cmd::send(
            id,
            message,
            await_completion,
            &config_dir,
            label,
            working_dir,
            policy,
        ),
        Command::Set {
            id,
            label,
            persona: _,
            working_dir,
            scope_exceptions,
        } => cmd::set(id, label, None, working_dir, scope_exceptions, &config_dir),
        Command::Delete(id) => cmd::delete(id, &config_dir),
        Command::Stop(id) => cmd::stop(id, &config_dir),
        Command::Kill(id) => cmd::stop(id, &config_dir),
        Command::InternalRun { id } => match cmd::internal_run(&id, &config_dir) {
            Ok(()) => Ok(serde_json::json!({"status": "ok"})),
            Err(e) => Err(e),
        },
        Command::ServerAction {
            action,
            profile,
            body_params,
        } => cmd::server_action(&action, profile.as_deref(), &body_params),
    };

    match result {
        Ok(json) => {
            if json.is_null() {
                return;
            }
            if let Err(e) = output::print_json(&json) {
                let err = JsonError::new("output_error", &e);
                let _ = output::print_error(&err);
                process::exit(1);
            }
        }
        Err(e) => {
            let err = JsonError::new("command_error", &e);
            let _ = output::print_error(&err);
            process::exit(1);
        }
    }
}
