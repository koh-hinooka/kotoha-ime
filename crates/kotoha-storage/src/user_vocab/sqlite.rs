//! SqliteUserVocabStore(Task B2〜B7 で実装)。

use std::sync::Arc;

use crate::database::Database;

pub struct SqliteUserVocabStore {
    #[allow(dead_code)] // B2〜B7 で使用開始
    pub(crate) db: Arc<Database>,
}
