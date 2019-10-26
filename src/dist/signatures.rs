//! Signature verification support for Rustup.

// TODO: Determine whether we want external keyring support

use pgp::types::KeyTrait;
use pgp::{Deserializable, StandaloneSignature};

use crate::config::PgpPublicKey;
use crate::errors::*;

use std::io::Read;

fn squish_internal_err<E: std::fmt::Display>(err: E) -> Error {
    ErrorKind::SignatureVerificationInternalError(format!("{}", err)).into()
}

pub fn verify_signature<T: Read>(
    mut content: T,
    signature: &str,
    keys: &[PgpPublicKey],
) -> Result<bool> {
    let mut content_buf = Vec::new();
    content.read_to_end(&mut content_buf)?;
    let (signatures, _) =
        StandaloneSignature::from_string_many(signature).map_err(squish_internal_err)?;

    for signature in signatures {
        let signature = signature.map_err(squish_internal_err)?;

        for key in keys {
            let actual_key = key.key();
            if actual_key.is_signing_key() && signature.verify(actual_key, &content_buf).is_ok() {
                return Ok(true);
            }

            for sub_key in &actual_key.public_subkeys {
                if sub_key.is_signing_key() && signature.verify(sub_key, &content_buf).is_ok() {
                    return Ok(true);
                }
            }
        }
    }

    Ok(false)
}
