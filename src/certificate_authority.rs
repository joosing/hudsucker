use crate::Error;
use chrono::{Duration, Utc};
use http::uri::Authority;
use moka::future::Cache;
use rcgen::{KeyPair, RcgenError, SanType};
use rustls::{NoClientAuth, ServerConfig};

/// Used to issue certificates for use when communicating with clients. Clients should trust the
/// provided certificate.
#[derive(Clone)]
pub struct CertificateAuthority {
    private_key: rustls::PrivateKey,
    ca_cert: rustls::Certificate,
    cache: Cache<Authority, ServerConfig>,
}

impl CertificateAuthority {
    pub fn new(
        private_key: rustls::PrivateKey,
        ca_cert: rustls::Certificate,
        cache_size: usize,
    ) -> Result<CertificateAuthority, Error> {
        let ca = CertificateAuthority {
            private_key,
            ca_cert,
            cache: Cache::new(cache_size),
        };

        ca.validate()?;
        Ok(ca)
    }

    pub(crate) async fn gen_server_config(&self, authority: &Authority) -> ServerConfig {
        if let Some(server_cfg) = self.cache.get(authority) {
            return server_cfg;
        }

        let mut server_cfg = ServerConfig::new(NoClientAuth::new());
        let certs = vec![self.gen_cert(authority)];

        server_cfg
            .set_single_cert(certs, self.private_key.clone())
            .expect("Failed to set certificate");
        server_cfg.set_protocols(&[b"http/1.1".to_vec()]);

        self.cache
            .insert(authority.clone(), server_cfg.clone())
            .await;

        server_cfg
    }

    fn gen_cert(&self, authority: &Authority) -> rustls::Certificate {
        let now = Utc::now();
        let mut params = rcgen::CertificateParams::default();
        params.not_before = now;
        params.not_after = now + Duration::weeks(52);
        params
            .subject_alt_names
            .push(SanType::DnsName(authority.host().to_string()));

        let key_pair = KeyPair::from_der(&self.private_key.0).expect("Failed to parse private key");
        params.alg = key_pair
            .compatible_algs()
            .next()
            .expect("Failed to find compatible algorithm");
        params.key_pair = Some(key_pair);

        let key_pair = KeyPair::from_der(&self.private_key.0).expect("Failed to parse private key");

        let ca_cert_params = rcgen::CertificateParams::from_ca_cert_der(&self.ca_cert.0, key_pair)
            .expect("Failed to parse CA certificate");
        let ca_cert = rcgen::Certificate::from_params(ca_cert_params)
            .expect("Failed to generate CA certificate");

        let cert = rcgen::Certificate::from_params(params).expect("Failed to generate certificate");
        rustls::Certificate(
            cert.serialize_der_with_signer(&ca_cert)
                .expect("Failed to serialize certificate"),
        )
    }

    fn validate(&self) -> Result<(), RcgenError> {
        let key_pair = rcgen::KeyPair::from_der(&self.private_key.0)?;
        rcgen::CertificateParams::from_ca_cert_der(&self.ca_cert.0, key_pair)?;
        Ok(())
    }
}
