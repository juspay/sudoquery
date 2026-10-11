pub mod chat;
pub mod event_description;
pub mod invitation;
pub mod live_dashboard;
pub mod membership;
pub mod message;
pub mod organization;
pub mod project;
pub mod property_description;
pub mod user;
pub mod user_project_console;

/// How many times a slug insert is retried with a freshly generated id
/// after a unique-constraint violation before giving up.
pub const MAX_SLUG_INSERT_ATTEMPTS: usize = 5;

/// Whether `err` is a unique-constraint violation (Postgres SQLSTATE 23505),
/// i.e. a generated slug collided with an existing row.
pub fn is_unique_violation(err: &sqlx::Error) -> bool {
    err.as_database_error()
        .is_some_and(|db_err| db_err.is_unique_violation())
}

pub use chat::*;
pub use event_description::*;
pub use invitation::*;
pub use live_dashboard::*;
pub use membership::*;
pub use message::*;
pub use organization::*;
pub use project::*;
pub use property_description::*;
pub use user::*;
pub use user_project_console::*;
