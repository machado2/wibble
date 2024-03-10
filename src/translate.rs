use indoc::indoc;
use sha2::Digest;
use sqlx::Row;
use tracing::{event, Level};
use uuid::Uuid;

use crate::error::Error;
use crate::llm::translate::Translate;
use crate::wibble_request::WibbleRequest;

mod private {
    use crate::error::Error;

    pub trait Internal {
        async fn try_translate(&self, english_text: &str) -> Result<String, Error>;
        async fn get_existing_translation(&self, hash: &str) -> Result<Option<String>, Error>;
        async fn get_lang_id(&self, lang: &str) -> Result<String, Error>;
    }
}

pub trait Translator {
    async fn translate(&self, english_text: &str) -> String;
}

impl private::Internal for WibbleRequest {
    async fn try_translate(&self, english_text: &str) -> Result<String, Error> {
        let hash = format!("{:x}", sha2::Sha256::digest(english_text.as_bytes()));
        let existing = self.get_existing_translation(&hash).await?;
        if let Some(translation) = existing {
            return Ok(translation);
        }

        let lang_id = self.get_lang_id(&self.lang).await?;
        match self.state.llm.translate(english_text, &self.lang).await {
            Ok(translation) => {
                let query = indoc! {r#"
                insert into translation (id, english_hash, lang_id, translation)
                values (?, ?, ?, ?) "#};
                let pool = &self.state.pool;
                let insert_result = sqlx::query(query)
                    .bind(Uuid::new_v4().to_string())
                    .bind(&hash)
                    .bind(&lang_id)
                    .bind(&translation)
                    .execute(pool)
                    .await;
                if let Err(error) = insert_result {
                    event!(Level::ERROR, "Error inserting translation: {}", error);
                }
                Ok(translation)
            }
            _ => Err(Error::NotFound),
        }
    }

    async fn get_existing_translation(&self, hash: &str) -> Result<Option<String>, Error> {
        async {
            let query = indoc! {r#"
                select t.translation
                from translation t
                         join language l
                              on t.lang_id = l.id
                where english_hash = ?
                  and l.name = ?
            "#};
            let pool = &self.state.pool;
            let r = sqlx::query(query)
                .bind(hash)
                .bind(&self.lang)
                .fetch_optional(pool)
                .await?;
            let translation = r.and_then(|r| r.try_get("translation").ok());
            Ok(translation)
        }
        .await
        .map_err(|e: sqlx::Error| {
            Error::Database(format!("Error getting existing translation {0}", e))
        })
    }

    async fn get_lang_id(&self, lang: &str) -> Result<String, Error> {
        async {
            let query = indoc! {r#"
            select id
            from language
            where name = ?
        "#};
            let pool = &self.state.pool;
            let id = sqlx::query(query)
                .bind(lang)
                .fetch_optional(pool)
                .await?
                .and_then(|r| r.try_get::<String, _>("id").ok());
            if let Some(id) = id {
                return Ok(id);
            }
            let id = Uuid::new_v4().to_string();
            let query = indoc! {r#"
            insert into language (id, name)
            values (?, ?)"#};
            sqlx::query(query)
                .bind(&id)
                .bind(lang)
                .execute(pool)
                .await?;
            Ok(id)
        }
        .await
        .map_err(|e: sqlx::Error| Error::Database(format!("Error getting lang id: {0}", e)))
    }
}

impl Translator for WibbleRequest {
    async fn translate(&self, english_text: &str) -> String {
        english_text.to_string()
        /* match self.try_translate(english_text).await {
            Ok(translated) => translated,
            _ => english_text.to_string(),
        }*/
    }
}
