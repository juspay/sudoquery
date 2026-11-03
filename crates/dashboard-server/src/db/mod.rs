pub mod chat;
pub mod invitation;
pub mod live_dashboard;
pub mod membership;
pub mod message;
pub mod organization;
pub mod user;
pub mod workspace;

/// Whether `err` is a unique-constraint violation (Postgres SQLSTATE 23505),
/// e.g. an org or workspace slug that is already taken.
pub fn is_unique_violation(err: &sqlx::Error) -> bool {
    err.as_database_error()
        .is_some_and(|db_err| db_err.is_unique_violation())
}

pub use chat::*;
pub use invitation::*;
pub use live_dashboard::*;
pub use membership::*;
pub use message::*;
pub use organization::*;
pub use user::*;
pub use workspace::*;
