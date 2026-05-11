use tracing::instrument;

#[instrument(name = "app.install_rustls_crypto_provider")]
pub fn install_rustls_crypto_provider() {
    // Ignore failure when another thread won the process-global provider race.
    let _ = rustls::crypto::ring::default_provider().install_default();
}
