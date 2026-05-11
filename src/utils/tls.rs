use std::sync::Once;

static RUSTLS_PROVIDER_INIT: Once = Once::new();

pub fn install_rustls_crypto_provider() {
    RUSTLS_PROVIDER_INIT.call_once(|| {
        // Ignore failure if a compatible process-global provider was installed elsewhere.
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}
