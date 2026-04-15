use sqlparser::ast::Statement;
use sqlparser::dialect::GenericDialect;
use sqlparser::parser::Parser;

pub enum QueryCheck {
    ReadOnly,
    Modifying(String),
    ParseError(String),
}

pub fn check_sql_readonly(sql: &str) -> QueryCheck {
    let dialect = GenericDialect {};

    let statements = match Parser::parse_sql(&dialect, sql) {
        Ok(stmts) => stmts,
        Err(e) => return QueryCheck::ParseError(e.to_string()),
    };

    if statements.is_empty() {
        return QueryCheck::ReadOnly;
    }

    for stmt in &statements {
        if let Some(reason) = is_modifying_statement(stmt) {
            return QueryCheck::Modifying(reason);
        }
    }

    QueryCheck::ReadOnly
}

fn is_modifying_statement(stmt: &Statement) -> Option<String> {
    match stmt {
        // Read-only statements
        Statement::Query(_) => None,
        Statement::Explain { .. } => None,
        Statement::ExplainTable { .. } => None,
        Statement::ShowFunctions { .. } => None,
        Statement::ShowVariable { .. } => None,
        Statement::ShowStatus { .. } => None,
        Statement::ShowVariables { .. } => None,
        Statement::ShowCreate { .. } => None,
        Statement::ShowColumns { .. } => None,
        Statement::ShowTables { .. } => None,
        Statement::ShowCollation { .. } => None,
        Statement::Use { .. } => None,
        Statement::StartTransaction { .. } => None,
        Statement::Commit { .. } => None,
        Statement::Rollback { .. } => None,
        Statement::Set(_) => None,

        // Data-modifying statements
        Statement::Insert(_) => Some("INSERT statement".to_string()),
        Statement::Update { .. } => Some("UPDATE statement".to_string()),
        Statement::Delete(_) => Some("DELETE statement".to_string()),
        Statement::Truncate { .. } => Some("TRUNCATE statement".to_string()),
        Statement::Merge { .. } => Some("MERGE statement".to_string()),

        // DDL statements
        Statement::CreateTable(_) => Some("CREATE TABLE statement".to_string()),
        Statement::CreateIndex(_) => Some("CREATE INDEX statement".to_string()),
        Statement::CreateView { .. } => Some("CREATE VIEW statement".to_string()),
        Statement::CreateSchema { .. } => Some("CREATE SCHEMA statement".to_string()),
        Statement::CreateDatabase { .. } => Some("CREATE DATABASE statement".to_string()),
        Statement::CreateFunction { .. } => Some("CREATE FUNCTION statement".to_string()),
        Statement::CreateProcedure { .. } => Some("CREATE PROCEDURE statement".to_string()),
        Statement::CreateTrigger { .. } => Some("CREATE TRIGGER statement".to_string()),
        Statement::CreateSequence { .. } => Some("CREATE SEQUENCE statement".to_string()),
        Statement::CreateRole { .. } => Some("CREATE ROLE statement".to_string()),

        Statement::AlterTable { .. } => Some("ALTER TABLE statement".to_string()),
        Statement::AlterIndex { .. } => Some("ALTER INDEX statement".to_string()),
        Statement::AlterView { .. } => Some("ALTER VIEW statement".to_string()),
        Statement::AlterRole { .. } => Some("ALTER ROLE statement".to_string()),

        Statement::Drop { .. } => Some("DROP statement".to_string()),
        Statement::DropFunction { .. } => Some("DROP FUNCTION statement".to_string()),
        Statement::DropProcedure { .. } => Some("DROP PROCEDURE statement".to_string()),

        // Permission statements
        Statement::Grant { .. } => Some("GRANT statement".to_string()),
        Statement::Revoke { .. } => Some("REVOKE statement".to_string()),

        // Other potentially dangerous statements
        Statement::Copy { .. } => Some("COPY statement".to_string()),
        Statement::Call(_) => Some("CALL statement (may modify data)".to_string()),
        Statement::Execute { .. } => Some("EXECUTE statement".to_string()),
        Statement::Prepare { .. } => Some("PREPARE statement".to_string()),

        // Unknown statements - be conservative and block
        _ => Some(format!(
            "Unknown statement type: {:?}",
            std::mem::discriminant(stmt)
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_readonly_select() {
        assert!(matches!(
            check_sql_readonly("SELECT * FROM users"),
            QueryCheck::ReadOnly
        ));
    }

    #[test]
    fn test_readonly_explain() {
        assert!(matches!(
            check_sql_readonly("EXPLAIN SELECT * FROM users"),
            QueryCheck::ReadOnly
        ));
    }

    #[test]
    fn test_readonly_show() {
        assert!(matches!(
            check_sql_readonly("SHOW TABLES"),
            QueryCheck::ReadOnly
        ));
        assert!(matches!(
            check_sql_readonly("SHOW CREATE TABLE users"),
            QueryCheck::ReadOnly
        ));
        assert!(matches!(
            check_sql_readonly("SHOW TABLES LIKE '%users%'"),
            QueryCheck::ReadOnly
        ));
    }

    #[test]
    fn test_modifying_insert() {
        assert!(matches!(
            check_sql_readonly("INSERT INTO users (name) VALUES ('test')"),
            QueryCheck::Modifying(_)
        ));
    }

    #[test]
    fn test_modifying_update() {
        assert!(matches!(
            check_sql_readonly("UPDATE users SET name = 'test'"),
            QueryCheck::Modifying(_)
        ));
    }

    #[test]
    fn test_modifying_delete() {
        assert!(matches!(
            check_sql_readonly("DELETE FROM users WHERE id = 1"),
            QueryCheck::Modifying(_)
        ));
    }

    #[test]
    fn test_modifying_drop() {
        assert!(matches!(
            check_sql_readonly("DROP TABLE users"),
            QueryCheck::Modifying(_)
        ));
    }

    #[test]
    fn test_modifying_truncate() {
        assert!(matches!(
            check_sql_readonly("TRUNCATE TABLE users"),
            QueryCheck::Modifying(_)
        ));
    }

    #[test]
    fn test_complex_explain() {
        let sql = r#"EXPLAIN SELECT ro.id, ro.service_id FROM routes ro WHERE ro.id = '2785'"#;
        assert!(matches!(check_sql_readonly(sql), QueryCheck::ReadOnly));
    }
}
