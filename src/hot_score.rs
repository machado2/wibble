use sea_orm::{DbBackend, Statement};

pub const HOT_SCORE_UPDATE_SQL: &str = r#"
UPDATE content
SET hot_score = (
    votes::double precision
    + (
        (1.0 / GREATEST(EXTRACT(EPOCH FROM (now() - created_at)) / 3600.0, 1.0))
        * 0.3
    )
)
WHERE generating = false AND flagged = false;
"#;

pub fn update_hot_score_statement() -> Statement {
    Statement::from_string(DbBackend::Postgres, HOT_SCORE_UPDATE_SQL.to_string())
}

#[cfg(test)]
mod tests {
    use super::HOT_SCORE_UPDATE_SQL;

    #[test]
    fn hot_score_uses_votes_instead_of_click_rate() {
        assert!(HOT_SCORE_UPDATE_SQL.contains("votes::double precision"));
        assert!(!HOT_SCORE_UPDATE_SQL.contains("click_count"));
        assert!(!HOT_SCORE_UPDATE_SQL.contains("impression_count"));
    }
}
