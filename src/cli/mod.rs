use std::collections::BTreeMap;

mod auth;
mod await_command;
mod config;
mod create;
mod delete;
mod get;
mod internal_run;
mod lifecycle;
mod list;
mod options;
mod parser;
mod send;
mod set;

pub mod output;
pub use output::{print_error, print_json};
pub use parser::{AuthSubcommand, Command, ConfigSubcommand};

pub fn parse_args(args: &[String]) -> Result<(Command, BTreeMap<String, Option<String>>), String> {
    let (filtered_args, global_flags) = options::extract(args);
    parser::parse(&filtered_args, global_flags)
}
