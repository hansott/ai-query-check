mod bash;
mod hook;
mod sql;

use serde::Serialize;
use std::io::{self, Read};
use std::process::ExitCode;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HookOutput {
    hook_specific_output: HookSpecificOutput,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HookSpecificOutput {
    hook_event_name: &'static str,
    decision: Decision,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Decision {
    behavior: &'static str,
}

fn main() -> ExitCode {
    let mut input = String::new();
    if io::stdin().read_to_string(&mut input).is_err() {
        return ExitCode::SUCCESS; // Normal permission prompt
    }

    let hook_input: hook::HookInput = match serde_json::from_str(&input) {
        Ok(h) => h,
        Err(_) => return ExitCode::SUCCESS, // Normal permission prompt
    };

    let command = &hook_input.tool_input.command;

    // Parse bash command and extract SQL from mysql -e arguments
    let sql_queries = match bash::extract_sql_from_bash(command) {
        Ok(queries) => queries,
        Err(e) => {
            eprintln!("[ai-query-check] Bash parse error: {}", e);
            return ExitCode::SUCCESS; // Normal permission prompt
        }
    };

    // Not a mysql command - normal permission prompt
    if sql_queries.is_empty() {
        return ExitCode::SUCCESS;
    }

    // Check each SQL query
    for query in &sql_queries {
        match sql::check_sql_readonly(query) {
            sql::QueryCheck::ReadOnly => {
                eprintln!(
                    "[ai-query-check] Auto-approving: {}",
                    truncate(query, 80)
                );
            }
            sql::QueryCheck::Modifying(reason) => {
                // Let user review modifying queries
                eprintln!(
                    "[ai-query-check] Requires approval: {} - {}",
                    reason,
                    truncate(query, 80)
                );
                return ExitCode::SUCCESS; // Normal permission prompt
            }
            sql::QueryCheck::ParseError(err) => {
                eprintln!(
                    "[ai-query-check] Parse error, requires approval: {}",
                    err
                );
                return ExitCode::SUCCESS; // Normal permission prompt
            }
        }
    }

    // All queries are read-only - auto-approve
    let output = HookOutput {
        hook_specific_output: HookSpecificOutput {
            hook_event_name: "PermissionRequest",
            decision: Decision {
                behavior: "allow",
            },
        },
    };
    println!("{}", serde_json::to_string(&output).unwrap());

    ExitCode::SUCCESS
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len])
    } else {
        s.to_string()
    }
}
