//! Signature verification support for Rustup.

// TODO: Determine whether we want external keyring support

use std::io::Read;

use anyhow::{Context, Result};
use pgp::types::KeyTrait;
use pgp::{Deserializable, StandaloneSignature};

use crate::config::PgpPublicKey;

pub(crate) fn verify_signature<T: Read>(
    mut content: T,
    signature: &str,
    keys: &[PgpPublicKey],
) -> Result<Option<usize>> {
    let mut content_buf = Vec::new();
    content.read_to_end(&mut content_buf)?;
    let (signatures, _) =
        StandaloneSignature::from_string_many(signature).context("error verifying signature")?;

    for signature in signatures {
        let signature = signature.context("error verifying signature")?;

        for (idx, key) in keys.iter().enumerate() {
            let actual_key = key.key();
            if actual_key.is_signing_key() && signature.verify(actual_key, &content_buf).is_ok() {
                return Ok(Some(idx));
            }

            for sub_key in &actual_key.public_subkeys {
                if sub_key.is_signing_key() && signature.verify(sub_key, &content_buf).is_ok() {
                    return Ok(Some(idx));
                }
            }
        }
    }

    Ok(None)
}
