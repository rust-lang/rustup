//! Signature verification support for Rustup.

// TODO: Determine whether we want external keyring support

use std::io::Read;

use anyhow::Result;

use sequoia_openpgp::{
    parse::{stream::*, Parse},
    policy, Cert, KeyHandle,
};

use crate::config::PgpPublicKey;

/// Returns the index of the cert in `certs` that verifies a
/// signature.
///
/// Ignores any signatures that are bad for any reason.  If no
/// signature could be verified, returns `None`.
// XXX: This is a bit of an odd policy.  Shouldn't we fail if we
// encounter a single bad signature (bad as in checksum doesn't check
// out, not bad as in we don't have the key)?
pub(crate) fn verify_signature<T: Read + Send + Sync>(
    content: T,
    signature: &str,
    certs: &[PgpPublicKey],
) -> Result<Option<usize>> {
    let p = policy::StandardPolicy::new();
    let helper = Helper::new(certs);
    let mut v = DetachedVerifierBuilder::from_reader(signature.as_bytes())?
        .with_policy(&p, None, helper)?;
    v.verify_reader(content)?;
    Ok(v.into_helper().index)
}

struct Helper<'a> {
    certs: &'a [PgpPublicKey],
    // The index of the cert in certs that successfully verified a
    // signature.
    index: Option<usize>,
}

impl<'a> Helper<'a> {
    fn new(certs: &'a [PgpPublicKey]) -> Self {
        Helper { certs, index: None }
    }
}

impl VerificationHelper for Helper<'_> {
    fn get_certs(&mut self, _: &[KeyHandle]) -> anyhow::Result<Vec<Cert>> {
        Ok(self.certs.iter().map(|c| c.cert().clone()).collect())
    }

    fn check(&mut self, structure: MessageStructure<'_>) -> anyhow::Result<()> {
        for layer in structure.into_iter() {
            match layer {
                MessageLayer::SignatureGroup { results } => {
                    for result in results {
                        match result {
                            Ok(GoodChecksum { ka, .. }) => {
                                // A good signature!  Find the index
                                // of the singer key and return
                                // success.
                                self.index = self.certs.iter().position(|c| c.cert() == ka.cert());
                                assert!(self.index.is_some());
                                return Ok(());
                            }
                            _ => {
                                // We ignore any errors.
                            }
                        }
                    }
                }
                MessageLayer::Compression { .. } => {
                    unreachable!("we're verifying detached signatures")
                }
                MessageLayer::Encryption { .. } => {
                    unreachable!("we're verifying detached signatures")
                }
            }
        }

        Ok(())
    }
}
