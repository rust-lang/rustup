//! Signature verification support for Rustup.
//!
//! Only compiled if the signature-check feature is enabled

// TODO: Determine whether we want external keyring support
// TODO: Determine how to integrate nicely into the test suite

use pgp::crypto::{HashAlgorithm, SymmetricKeyAlgorithm};
use pgp::types::{CompressionAlgorithm, KeyTrait};
use pgp::{Deserializable, SignedPublicKey, SignedSecretKey, StandaloneSignature};
use pgp::{KeyType, Message, SecretKeyParamsBuilder};

use crate::errors::*;

const SIGNING_KEY_BYTES: &[u8] = include_bytes!("rust-signing-key.asc");

lazy_static::lazy_static! {
    static ref SIGNING_KEYS: Vec<SignedPublicKey> = load_keys().expect("invalid keys");
}

fn squish_internal_err<E: std::fmt::Display>(err: E) -> Error {
    ErrorKind::SignatureVerificationInternalError(format!("{}", err)).into()
}

fn load_keys() -> Result<Vec<SignedPublicKey>> {
    SignedPublicKey::from_armor_many(std::io::Cursor::new(SIGNING_KEY_BYTES))
        .map_err(squish_internal_err)?
        .0
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(squish_internal_err)
}

pub fn verify_signature(content: &str, signature: &str) -> Result<bool> {
    let (signatures, _) =
        StandaloneSignature::from_string_many(signature).map_err(squish_internal_err)?;

    for signature in signatures {
        let signature = signature.map_err(squish_internal_err)?;

        for key in &*SIGNING_KEYS {
            if key.is_signing_key() {
                if signature.verify(key, content.as_bytes()).is_ok() {
                    return Ok(true);
                }
            }
            for sub_key in &key.public_subkeys {
                if sub_key.is_signing_key() {
                    if signature.verify(sub_key, content.as_bytes()).is_ok() {
                        return Ok(true);
                    }
                }
            }
        }
    }

    Ok(false)
}

fn generate_key() -> std::result::Result<SignedSecretKey, pgp::errors::Error> {
    let key_params = SecretKeyParamsBuilder::default()
        .key_type(KeyType::EdDSA)
        .can_sign(true)
        .primary_user_id("Me-X <me-x25519@mail.com>".into())
        .passphrase(None)
        .preferred_symmetric_algorithms(
            vec![
                SymmetricKeyAlgorithm::AES256,
                SymmetricKeyAlgorithm::AES192,
                SymmetricKeyAlgorithm::AES128,
            ]
            .into(),
        )
        .preferred_hash_algorithms(
            vec![
                HashAlgorithm::SHA2_256,
                HashAlgorithm::SHA2_384,
                HashAlgorithm::SHA2_512,
                HashAlgorithm::SHA2_224,
                HashAlgorithm::SHA1,
            ]
            .into(),
        )
        .preferred_compression_algorithms(
            vec![CompressionAlgorithm::ZLIB, CompressionAlgorithm::ZIP].into(),
        )
        .build()
        .unwrap();

    let key = key_params.generate()?;
    key.sign(|| "".into())
}

fn sign_data(
    data: &[u8],
    key: &SignedSecretKey,
) -> std::result::Result<StandaloneSignature, pgp::errors::Error> {
    let msg = Message::new_literal_bytes("message", data);
    let signed_message = msg.sign(key, || "".into(), HashAlgorithm::SHA2_256)?;
    let sig = signed_message.into_signature();

    Ok(sig)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_signature() {
        let content = include_str!("../../tests/data/channel-rust-stable.toml");
        let signature = include_str!("../../tests/data/channel-rust-stable.toml.asc");

        assert!(
            verify_signature(content, signature).unwrap(),
            "invalid signature"
        );
    }

    #[test]
    fn test_sign_verify() {
        let content = "hello world";

        let key = generate_key().unwrap();
        let signed_message = sign_data(content.as_bytes(), &key).unwrap();

        // generate ascii armored version of the signature
        let signature_str = signed_message.to_armored_string(None).unwrap();

        let (signature, _) = StandaloneSignature::from_string(&signature_str).unwrap();
        assert!(key.is_signing_key());

        signature
            .verify(&key, content.as_bytes())
            .expect("invalid signature");
    }
}
