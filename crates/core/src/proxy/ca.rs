//! Local certificate authority. Generated once per machine, stored in the
//! proxy folder, and only ever added to Roblox's own `ssl/cacert.pem` —
//! never to the Windows certificate store.

use anyhow::{Context, Result};
use parking_lot::Mutex;
use rcgen::{
    BasicConstraints, Certificate, CertificateParams, DistinguishedName, DnType, ExtendedKeyUsagePurpose, IsCa,
    KeyPair, KeyUsagePurpose, SanType,
};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use rustls::ServerConfig;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use time::{Duration, OffsetDateTime};

pub struct CertAuthority {
    cert: Certificate,
    key: KeyPair,
    pem: String,
    der: CertificateDer<'static>,
    leaves: Mutex<HashMap<String, Arc<ServerConfig>>>,
}

impl CertAuthority {
    /// Load the CA from `dir`, creating it on first use.
    pub fn load_or_create(dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(dir)?;
        let cert_path = dir.join("ca.pem");
        let key_path = dir.join("ca.key.pem");
        if let (Ok(c), Ok(k)) = (std::fs::read_to_string(&cert_path), std::fs::read_to_string(&key_path)) {
            if let Ok(ca) = Self::from_pem(&c, &k) {
                return Ok(ca);
            }
        }
        let (cert, key) = Self::generate()?;
        std::fs::write(&cert_path, cert.pem())?;
        std::fs::write(&key_path, key.serialize_pem())?;
        Self::from_pem(&cert.pem(), &key.serialize_pem())
    }

    fn generate() -> Result<(Certificate, KeyPair)> {
        let mut p = CertificateParams::default();
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, "Arcadia Local Proxy CA");
        dn.push(DnType::OrganizationName, "Arcadia (this computer only)");
        p.distinguished_name = dn;
        p.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
        p.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign, KeyUsagePurpose::DigitalSignature];
        let now = OffsetDateTime::now_utc();
        p.not_before = now - Duration::days(1);
        p.not_after = now + Duration::days(3650);
        let key = KeyPair::generate()?;
        let cert = p.self_signed(&key)?;
        Ok((cert, key))
    }

    pub fn from_pem(cert_pem: &str, key_pem: &str) -> Result<Self> {
        let key = KeyPair::from_pem(key_pem).context("CA key")?;
        let params = CertificateParams::from_ca_cert_pem(cert_pem).context("CA cert")?;
        // Re-signing with the same key and name yields an equivalent issuer;
        // we still serve the original DER so chains match what clients trust.
        let cert = params.self_signed(&key)?;
        let der = rustls_pemfile::certs(&mut cert_pem.as_bytes())
            .next()
            .context("no certificate in CA PEM")??;
        Ok(Self { cert, key, pem: cert_pem.to_string(), der, leaves: Mutex::new(HashMap::new()) })
    }

    pub fn pem(&self) -> &str {
        &self.pem
    }

    /// TLS config presenting a certificate for `host`, signed by this CA.
    pub fn server_config(&self, host: &str) -> Result<Arc<ServerConfig>> {
        if let Some(c) = self.leaves.lock().get(host) {
            return Ok(c.clone());
        }
        let mut p = CertificateParams::new(vec![host.to_string()])?;
        if let Ok(ip) = host.parse::<std::net::IpAddr>() {
            p.subject_alt_names = vec![SanType::IpAddress(ip)];
        }
        p.distinguished_name.push(DnType::CommonName, host);
        p.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        let now = OffsetDateTime::now_utc();
        p.not_before = now - Duration::days(1);
        p.not_after = now + Duration::days(365);
        let leaf_key = KeyPair::generate()?;
        let leaf = p.signed_by(&leaf_key, &self.cert, &self.key)?;

        let chain = vec![leaf.der().clone(), self.der.clone()];
        let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(leaf_key.serialize_der()));
        let mut cfg = ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()?
            .with_no_client_auth()
            .with_single_cert(chain, key)?;
        cfg.alpn_protocols = vec![b"http/1.1".to_vec()];
        let cfg = Arc::new(cfg);
        self.leaves.lock().insert(host.to_string(), cfg.clone());
        Ok(cfg)
    }
}
