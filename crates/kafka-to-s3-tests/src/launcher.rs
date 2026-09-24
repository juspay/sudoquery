//! Launcher seam for the service under test.

/// Deliberate no-op launcher for the `kafka_to_s3` service.
///
/// The real service runs **externally**: the operator starts it (bare binary
/// or container) with configuration matching the harness's `K2S_TEST_*`
/// environment variables before running the tests, and stops it afterwards.
/// This launcher therefore starts nothing.
///
/// A future implementation could spawn the service binary or container here
/// (e.g. via [`std::process::Command`] or a docker API client) and return a
/// [`ServiceHandle`] that stops it on drop; tests call
/// [`StubLauncher::launch`] at a single call-site, so swapping in that
/// implementation stays mechanical.
pub struct StubLauncher;

impl StubLauncher {
    /// Returns a [`ServiceHandle`] without starting anything.
    ///
    /// Tests hold the handle for their whole body so the call-site shape
    /// matches a future launcher that manages the service lifecycle.
    pub fn launch(&self) -> ServiceHandle {
        ServiceHandle
    }
}

/// Handle to the externally-run service under test.
///
/// Dropping it does nothing: the operator, not the harness, owns the
/// service's lifecycle.
pub struct ServiceHandle;
