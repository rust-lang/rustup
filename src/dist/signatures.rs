//! Signature verification support for Rustup.
//!
//! Only compiled if the signature-check feature is enabled

// TODO: Determine whether we want external keyring support
// TODO: Determine how to integrate nicely into the test suite

use pgp::types::KeyTrait;
use pgp::{Deserializable, SignedPublicKey, StandaloneSignature};

use crate::errors::*;

// const SIGNING_KEY_BYTES: &[u8] = include_bytes!("rust-signing-key.asc");
const SIGNING_KEY_BYTES: &[u8] = include_bytes!("../../tests/mock/signing-key.pub.asc");

lazy_static::lazy_static! {
    static ref SIGNING_KEYS: Vec<SignedPublicKey> = {
        pgp::SignedPublicKey::from_armor_many(std::io::Cursor::new(SIGNING_KEY_BYTES))
            .map_err(squish_internal_err).unwrap()
            .0
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(squish_internal_err).unwrap()
    };
}

fn squish_internal_err<E: std::fmt::Display>(err: E) -> Error {
    ErrorKind::SignatureVerificationInternalError(format!("{}", err)).into()
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
}
