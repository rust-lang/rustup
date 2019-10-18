//! Signature verification support for Rustup.
//!
//! Only compiled if the signature-check feature is enabled

// TODO: Decide if all the eprintln!() need converting to notifications
// or if we're happy as they are.
// TODO: Determine whether we want external keyring support
// TODO: Determine how to integrate nicely into the test suite

use pgp::composed::{Deserializable, Message, SignedPublicKey};

use crate::errors::*;

const SIGNING_KEY_BYTES: &[u8] = include_bytes!("rust-signing-key.asc");

lazy_static::lazy_static! {
    static ref SIGNING_KEYS: Vec<SignedPublicKey> = load_keys().expect("invalid");
}

fn squish_internal_err<E: std::fmt::Display>(err: E) -> Error {
    ErrorKind::SignatureVerificationInternalError(format!("{}", err)).into()
}

fn load_keys() -> Result<Vec<SignedPublicKey>> {
    let signing_key = SignedPublicKey::from_armor_many(std::io::Cursor::new(SIGNING_KEY_BYTES))
        .map_err(squish_internal_err)?
        .0
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(squish_internal_err)?;

    Ok(signing_key)
}

pub fn verify_signature(content: &str, signature: &str) -> Result<bool> {
    // TODO: implement actual signature + content verification

    let (messages, _) = Message::from_string_many(content).map_err(squish_internal_err)?;

    let mut good = 0;
    for message in messages {
        let message = message.map_err(squish_internal_err)?;
        if message.verify(&SIGNING_KEYS[0]).is_ok() {
            good += 1;
        }
    }

    Ok(good > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_keys() {
        assert_eq!(SIGNING_KEYS.len(), 1, "failed to load keys");
    }
}
