use std::path::{Path, PathBuf};

use axum_server::tls_rustls::RustlsConfig;
use rcgen::{generate_simple_self_signed, CertifiedKey};

/// Browsers only allow microphone access (`getUserMedia`) on a "secure
/// context" — HTTPS, or `localhost`. Since the whole point of this server is
/// to be reachable from other devices on the LAN (not just localhost), it
/// needs HTTPS even for purely local/personal use. A self-signed cert still
/// triggers a one-time "not secure" warning the user has to click through
/// per browser/device, but that's the standard, expected experience for a
/// self-hosted LAN service without a real domain + CA-signed cert.
pub async fn load_or_create(cert_dir: &Path) -> RustlsConfig {
    let cert_path = cert_dir.join("cert.pem");
    let key_path = cert_dir.join("key.pem");

    if !cert_path.exists() || !key_path.exists() {
        std::fs::create_dir_all(cert_dir).expect("failed to create TLS cert directory");
        let (cert_pem, key_pem) = generate_cert_pem();
        std::fs::write(&cert_path, cert_pem).expect("failed to write TLS certificate");
        std::fs::write(&key_path, key_pem).expect("failed to write TLS private key");
        println!("Generated a new self-signed TLS certificate at {cert_dir:?}");
    }

    RustlsConfig::from_pem_file(&cert_path, &key_path)
        .await
        .expect("failed to load TLS certificate")
}

fn generate_cert_pem() -> (String, String) {
    let mut subject_alt_names = vec![
        "localhost".to_string(),
        "127.0.0.1".to_string(),
        "::1".to_string(),
    ];
    if let Ok(lan_ip) = local_ip_address::local_ip() {
        subject_alt_names.push(lan_ip.to_string());
    }

    let CertifiedKey { cert, signing_key } = generate_simple_self_signed(subject_alt_names)
        .expect("failed to generate self-signed certificate");

    (cert.pem(), signing_key.serialize_pem())
}

pub fn cert_dir_from_app_data(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("tls")
}
