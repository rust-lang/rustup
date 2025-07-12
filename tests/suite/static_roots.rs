use std::sync::{Arc, Mutex};

use rustls::{
    DigitallySignedStruct, Error, RootCertStore, SignatureScheme,
    client::{
        WebPkiServerVerifier,
        danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    },
    crypto::{CryptoProvider, aws_lc_rs},
    pki_types::{CertificateDer, ServerName, TrustAnchor, UnixTime},
};
use webpki::EndEntityCert;
use webpki_roots::TLS_SERVER_ROOTS;

#[test]
fn store_static_roots() {
    let provider = Arc::new(aws_lc_rs::default_provider());
    let mut roots = RootCertStore::empty();
    roots.extend(TLS_SERVER_ROOTS.iter().cloned());
    let roots = Arc::new(roots);
    let inner = WebPkiServerVerifier::builder_with_provider(roots.clone(), provider.clone())
        .build()
        .unwrap();

    let verifier = Arc::new(TrackRootVerifier {
        root: Mutex::default(),
        roots,
        inner,
        provider: provider.clone(),
    });

    let config = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .dangerous()
        .with_custom_certificate_verifier(verifier.clone())
        .with_no_client_auth();

    for host in HOSTS {
        
    }
}

const HOSTS: &[&str] = &[
    "fastly-static.rust-lang.org",
    "cloudfront-static.rust-lang.org",
];

#[derive(Debug)]
struct TrackRootVerifier {
    root: Mutex<Option<TrustAnchor<'static>>>,
    inner: Arc<WebPkiServerVerifier>,
    roots: Arc<RootCertStore>,
    provider: Arc<CryptoProvider>,
}

impl ServerCertVerifier for TrackRootVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        let verified = self.inner.verify_server_cert(
            end_entity,
            intermediates,
            server_name,
            ocsp_response,
            now,
        )?;

        let cert = EndEntityCert::try_from(end_entity)
            .map_err(|e| Error::General(format!("invalid end entity certificate: {e}")))?;

        let path = cert
            .verify_for_usage(
                &self.provider.signature_verification_algorithms.all,
                &self.roots.roots,
                intermediates,
                now,
                webpki::KeyUsage::server_auth(),
                None,
                None,
            )
            .unwrap();

        let mut root = self.root.lock().unwrap();
        *root = Some(path.anchor().to_owned());
        Ok(verified)
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        self.inner.verify_tls12_signature(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        self.inner.verify_tls13_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.inner.supported_verify_schemes()
    }
}
