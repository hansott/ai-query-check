use brush_parser::{ast, Parser, ParserOptions, SourceInfo};

/// Remove only the outer quotes from a bash word, preserving inner quotes.
/// In bash, single quotes inside double quotes are literal characters.
fn unquote_outer(s: &str) -> String {
    let s = s.trim();
    if s.len() < 2 {
        return s.to_string();
    }

    let first = s.chars().next().unwrap();
    let last = s.chars().last().unwrap();

    if (first == '"' && last == '"') || (first == '\'' && last == '\'') {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

/// Extract SQL queries from bash commands containing mysql -e "..."
pub fn extract_sql_from_bash(command: &str) -> Result<Vec<String>, String> {
    let options = ParserOptions::default();
    let source_info = SourceInfo::default();
    let mut parser = Parser::new(std::io::Cursor::new(command), &options, &source_info);

    let program = parser
        .parse_program()
        .map_err(|e| format!("Failed to parse bash: {:?}", e))?;

    let mut sql_queries = Vec::new();
    extract_sql_from_program(&program, &mut sql_queries);

    Ok(sql_queries)
}

fn extract_sql_from_program(program: &ast::Program, queries: &mut Vec<String>) {
    for complete_command in &program.complete_commands {
        extract_sql_from_compound_list(complete_command, queries);
    }
}

fn extract_sql_from_compound_list(list: &ast::CompoundList, queries: &mut Vec<String>) {
    for item in &list.0 {
        // CompoundListItem is a tuple struct: (AndOrList, SeparatorOperator)
        extract_sql_from_and_or_list(&item.0, queries);
    }
}

fn extract_sql_from_and_or_list(list: &ast::AndOrList, queries: &mut Vec<String>) {
    extract_sql_from_pipeline(&list.first, queries);
    for additional in &list.additional {
        // AndOr is an enum: And(Pipeline) or Or(Pipeline)
        let pipeline = match additional {
            ast::AndOr::And(p) | ast::AndOr::Or(p) => p,
        };
        extract_sql_from_pipeline(pipeline, queries);
    }
}

fn extract_sql_from_pipeline(pipeline: &ast::Pipeline, queries: &mut Vec<String>) {
    for command in &pipeline.seq {
        extract_sql_from_command(command, queries);
    }
}

fn extract_sql_from_command(command: &ast::Command, queries: &mut Vec<String>) {
    match command {
        ast::Command::Simple(simple) => {
            extract_sql_from_simple_command(simple, queries);
        }
        ast::Command::Compound(compound, _) => {
            extract_sql_from_compound_command(compound, queries);
        }
        _ => {}
    }
}

fn extract_sql_from_compound_command(compound: &ast::CompoundCommand, queries: &mut Vec<String>) {
    match compound {
        ast::CompoundCommand::BraceGroup(cmd) => {
            extract_sql_from_compound_list(&cmd.list, queries);
        }
        ast::CompoundCommand::Subshell(cmd) => {
            extract_sql_from_compound_list(&cmd.list, queries);
        }
        ast::CompoundCommand::WhileClause(cmd) | ast::CompoundCommand::UntilClause(cmd) => {
            // WhileOrUntilClauseCommand is a tuple: (condition, body, loc)
            extract_sql_from_compound_list(&cmd.0, queries);
            extract_sql_from_compound_list(&cmd.1.list, queries);
        }
        ast::CompoundCommand::IfClause(cmd) => {
            extract_sql_from_compound_list(&cmd.condition, queries);
            extract_sql_from_compound_list(&cmd.then, queries);
            if let Some(elses) = &cmd.elses {
                for else_clause in elses {
                    if let Some(condition) = &else_clause.condition {
                        extract_sql_from_compound_list(condition, queries);
                    }
                    extract_sql_from_compound_list(&else_clause.body, queries);
                }
            }
        }
        ast::CompoundCommand::ForClause(cmd) => {
            extract_sql_from_compound_list(&cmd.body.list, queries);
        }
        ast::CompoundCommand::ArithmeticForClause(cmd) => {
            extract_sql_from_compound_list(&cmd.body.list, queries);
        }
        ast::CompoundCommand::CaseClause(cmd) => {
            for case_item in &cmd.cases {
                if let Some(body) = &case_item.cmd {
                    extract_sql_from_compound_list(body, queries);
                }
            }
        }
        ast::CompoundCommand::Arithmetic(_) => {
            // No SQL extraction needed for arithmetic
        }
    }
}

fn extract_sql_from_simple_command(cmd: &ast::SimpleCommand, queries: &mut Vec<String>) {
    // Collect all words from the command (name + suffix)
    let mut words: Vec<String> = Vec::new();

    if let Some(word_or_name) = &cmd.word_or_name {
        words.push(unquote_outer(&word_or_name.value));
    }

    if let Some(suffix) = &cmd.suffix {
        for item in &suffix.0 {
            if let ast::CommandPrefixOrSuffixItem::Word(word) = item {
                words.push(unquote_outer(&word.value));
            }
        }
    }

    // Check if this is a mysql or psql command
    let is_mysql = words.iter().any(|w| w == "mysql" || w.ends_with("/mysql"));
    let is_psql = words.iter().any(|w| w == "psql" || w.ends_with("/psql"));

    if is_mysql {
        extract_mysql_queries(&words, queries);
    } else if is_psql {
        extract_psql_queries(&words, queries);
    }
}

fn extract_mysql_queries(words: &[String], queries: &mut Vec<String>) {
    let mut i = 0;
    while i < words.len() {
        let word = &words[i];

        if word == "-e" || word == "--execute" {
            if i + 1 < words.len() {
                queries.push(words[i + 1].clone());
            }
            i += 2;
        } else if let Some(sql) = word.strip_prefix("-e") {
            if !sql.is_empty() {
                queries.push(sql.to_string());
            }
            i += 1;
        } else if let Some(sql) = word.strip_prefix("--execute=") {
            if !sql.is_empty() {
                queries.push(unquote_outer(sql));
            }
            i += 1;
        } else {
            i += 1;
        }
    }
}

fn extract_psql_queries(words: &[String], queries: &mut Vec<String>) {
    let mut i = 0;
    while i < words.len() {
        let word = &words[i];

        if word == "-c" || word == "--command" {
            if i + 1 < words.len() {
                queries.push(words[i + 1].clone());
            }
            i += 2;
        } else if let Some(sql) = word.strip_prefix("-c") {
            if !sql.is_empty() {
                queries.push(sql.to_string());
            }
            i += 1;
        } else if let Some(sql) = word.strip_prefix("--command=") {
            if !sql.is_empty() {
                queries.push(unquote_outer(sql));
            }
            i += 1;
        } else {
            i += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_sql_simple() {
        let cmd = r#"mysql -e "SELECT * FROM users""#;
        let queries = extract_sql_from_bash(cmd).unwrap();
        assert_eq!(queries, vec!["SELECT * FROM users"]);
    }

    #[test]
    fn test_extract_sql_docker() {
        let cmd =
            r#"docker exec mysql-container mysql -uroot -p123 mydb -e "SELECT id FROM users""#;
        let queries = extract_sql_from_bash(cmd).unwrap();
        assert_eq!(queries, vec!["SELECT id FROM users"]);
    }

    #[test]
    fn test_extract_sql_single_quotes() {
        let cmd = "mysql -e 'SHOW TABLES'";
        let queries = extract_sql_from_bash(cmd).unwrap();
        assert_eq!(queries, vec!["SHOW TABLES"]);
    }

    #[test]
    fn test_extract_sql_not_mysql() {
        let cmd = "ls -la";
        let queries = extract_sql_from_bash(cmd).unwrap();
        assert!(queries.is_empty());
    }

    #[test]
    fn test_pipe_command() {
        let cmd = r#"docker exec mysql mysql -e "SELECT * FROM users" | grep admin"#;
        let queries = extract_sql_from_bash(cmd).unwrap();
        assert_eq!(queries, vec!["SELECT * FROM users"]);
    }

    #[test]
    fn test_execute_flag_long() {
        let cmd = r#"mysql --execute="SELECT 1""#;
        let queries = extract_sql_from_bash(cmd).unwrap();
        assert_eq!(queries, vec!["SELECT 1"]);
    }

    #[test]
    fn test_psql_simple() {
        let cmd = r#"psql -c "SELECT * FROM users""#;
        let queries = extract_sql_from_bash(cmd).unwrap();
        assert_eq!(queries, vec!["SELECT * FROM users"]);
    }

    #[test]
    fn test_psql_docker() {
        let cmd = r#"docker exec postgres psql -U user -d db -c "SELECT id FROM users""#;
        let queries = extract_sql_from_bash(cmd).unwrap();
        assert_eq!(queries, vec!["SELECT id FROM users"]);
    }

    #[test]
    fn test_psql_command_flag_long() {
        let cmd = r#"psql --command="EXPLAIN SELECT 1""#;
        let queries = extract_sql_from_bash(cmd).unwrap();
        assert_eq!(queries, vec!["EXPLAIN SELECT 1"]);
    }

    #[test]
    fn test_mysql_show_tables_like() {
        let cmd = r#"docker exec aikido-core-mysql mysql -uroot -p123 aikido -e "SHOW TABLES LIKE '%zen%';""#;
        let queries = extract_sql_from_bash(cmd).unwrap();
        assert_eq!(queries, vec!["SHOW TABLES LIKE '%zen%';"]);
    }
}
