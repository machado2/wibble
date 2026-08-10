use chrono::NaiveDateTime;
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

pub fn calculate_hot_score(votes: i32, created_at: NaiveDateTime, now: NaiveDateTime) -> f64 {
    let age_seconds = (now - created_at).num_seconds().max(0) as f64;
    let age_hours = (age_seconds / 3600.0).max(1.0);
    votes as f64 + ((1.0 / age_hours) * 0.3)
}

#[cfg(test)]
mod tests {
    use super::{calculate_hot_score, HOT_SCORE_UPDATE_SQL};

    #[test]
    fn hot_score_uses_votes_instead_of_click_rate() {
        assert!(HOT_SCORE_UPDATE_SQL.contains("votes::double precision"));
        assert!(!HOT_SCORE_UPDATE_SQL.contains("click_count"));
        assert!(!HOT_SCORE_UPDATE_SQL.contains("impression_count"));
    }

    #[test]
    fn hot_score_decreases_as_article_ages() {
        let created_at = chrono::NaiveDate::from_ymd_opt(2026, 4, 18)
            .unwrap()
            .and_hms_opt(10, 0, 0)
            .unwrap();
        let early = calculate_hot_score(3, created_at, created_at);
        let later = calculate_hot_score(3, created_at, created_at + chrono::TimeDelta::hours(6));
        assert!(early > later);
    }
}
