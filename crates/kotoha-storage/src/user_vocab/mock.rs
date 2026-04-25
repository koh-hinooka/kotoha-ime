//! MockUserVocabStore(Task B9 で実装)。

use std::sync::Mutex;

use crate::user_vocab::store::UserVocabRecord;

pub struct MockUserVocabStore {
    #[allow(dead_code)] // B9 で使用開始
    pub(crate) records: Mutex<Vec<UserVocabRecord>>,
    #[allow(dead_code)] // B9 で使用開始
    pub(crate) next_id: Mutex<i64>,
}
