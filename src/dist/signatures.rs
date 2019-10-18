//! Signature verification support for Rustup.
//!
//! Only compiled if the signature-check feature is enabled

// TODO: Decide if all the eprintln!() need converting to notifications
// or if we're happy as they are.
// TODO: Determine whether we want external keyring support
// TODO: Determine how to integrate nicely into the test suite

use pgp::{Deserializable, Signature, SignedPublicKey};

use crate::errors::*;

const SIGNING_KEY_BYTES: &[u8] = include_bytes!("rust-signing-key.asc");

lazy_static::lazy_static! {
    static ref SIGNING_KEY: SignedPublicKey = load_key().expect("invalid");
}

fn squish_internal_err<E: std::fmt::Display>(err: E) -> Error {
    ErrorKind::SignatureVerificationInternalError(format!("{}", err)).into()
}

fn load_key() -> Result<SignedPublicKey> {
    let (signing_key, _) =
        SignedPublicKey::from_armor_single(std::io::Cursor::new(SIGNING_KEY_BYTES))
            .map_err(squish_internal_err)?;

    Ok(signing_key)
}

pub fn verify_signature(content: &str, signature: &str) -> Result<bool> {
    let (signatures, _) = Signature::from_string_many(signature).map_err(squish_internal_err)?;

    for signature in signatures {
        let signature = signature.map_err(squish_internal_err)?;
        println!("{:#?}", &signature);

        if signature.verify(&*SIGNING_KEY, content.as_bytes()).is_ok() {
            return Ok(true);
        }
        for sub_key in &SIGNING_KEY.public_subkeys {
            if signature.verify(sub_key, content.as_bytes()).is_ok() {
                return Ok(true);
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
