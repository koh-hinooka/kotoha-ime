//! `UserVocabStore` trait + `UserVocabRecord` + `Sqlite/Mock` 実装。

pub mod mock;
pub mod sqlite;
pub mod store;

pub use mock::MockUserVocabStore;
pub use sqlite::SqliteUserVocabStore;
pub use store::{UserVocabRecord, UserVocabStore};
