//! clickhouse-proxy: everything ClickHouse, behind one HTTP API (see
//! [`server`]) that dashboard-server and the CDP dashboard both call.
//!
//! - **Queries** run as each workspace's own ClickHouse user, in its own
//!   database ([`clickhouse`]).
//! - **Workspace objects** — the database fed from the workspace's Kafka
//!   topic, and its read-only user — are created and dropped here
//!   ([`provisioning`]).
//! - **Its own tables** in Postgres hold saved SQL consoles and event and
//!   property descriptions ([`store`]); it creates its schema and runs its
//!   migrations itself at startup.
//!
//! [`client`] is the Rust client the other services use.

pub mod clickhouse;
pub mod client;
pub mod provisioning;
pub mod server;
pub mod store;
