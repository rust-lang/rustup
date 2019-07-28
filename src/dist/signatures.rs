//! Signature verification support for Rustup.
//!
//! Only compiled if the signature-check feature is enabled

// TODO: Determine how safe it is to use this, because we took a large
// amount of this code from `sqv`
// TODO: Decide if all the eprintln!() need converting to notifications
// or if we're happy as they are.
// TODO: Determine whether we want external keyring support
// TODO: Determine how to integrate nicely into the test suite

use sequoia_openpgp as openpgp;

use self::openpgp::constants::HashAlgorithm;
use self::openpgp::crypto::hash::Hash;
use self::openpgp::parse::{PacketParser, PacketParserResult, Parse};
use self::openpgp::tpk::TPKParser;
use self::openpgp::{packet::Signature, KeyID, Packet, RevocationStatus, TPK};

use crate::errors::*;

use std::collections::{HashMap, HashSet};

const SIGNING_KEY: &[u8] = include_bytes!("rust-signing-key.asc");

fn squish_internal_err<E: std::fmt::Display>(err: E) -> Error {
    ErrorKind::SignatureVerificationInternalError(format!("{}", err)).into()
}

fn load_keys() -> Result<Vec<TPK>> {
    TPKParser::from_bytes(SIGNING_KEY)
        .map_err(squish_internal_err)?
        .map(|tpkr| match tpkr {
            Ok(tpk) => Ok(tpk),
            Err(err) => {
                let err = format!("{}", err);
                Err(ErrorKind::SignatureVerificationInternalError(err).into())
            }
        })
        .collect()
}

fn parse_signatures(content: &str) -> Result<Vec<(Signature, KeyID, Option<TPK>)>> {
    let mut sigs = Vec::new();
    let mut sigs_seen = HashSet::new();
    let mut ppr = PacketParser::from_bytes(content.as_bytes()).map_err(squish_internal_err)?;

    while let PacketParserResult::Some(pp) = ppr {
        let (packet, ppr_tmp) = pp.recurse().unwrap();
        ppr = ppr_tmp;

        match packet {
            Packet::Signature(sig) => {
                // To check for duplicates, we normalize the
                // signature, and put it into the hashset of seen
                // signatures.
                let mut sig_normalized = sig.clone();
                sig_normalized.unhashed_area_mut().clear();
                if sigs_seen.replace(sig_normalized).is_some() {
                    eprintln!("Ignoring duplicate signature.");
                    continue;
                }

                if let Some(fp) = sig.issuer_fingerprint() {
                    // XXX: We use a KeyID even though we have a
                    // fingerprint!
                    sigs.push((sig, fp.to_keyid(), None));
                } else if let Some(keyid) = sig.issuer() {
                    sigs.push((sig, keyid, None));
                } else {
                    eprintln!(
                        "One or more signatures do not contain information \
                         about the issuer.  Unable to validate."
                    );
                }
            }
            Packet::CompressedData(_) => {
                // Skip it.
            }
            packet => {
                Err(squish_internal_err(format!(
                    "OpenPGP message is not a detached signature.  \
                     Encountered unexpected packet: {:?} packet.",
                    packet.tag()
                )))?;
            }
        }
    }

    Ok(sigs)
}

fn tpk_has_key(tpk: &TPK, keyid: &KeyID) -> bool {
    // Even if a key is revoked or expired, we can still use it to
    // verify a message.
    tpk.keys_all().any(|(_, _, k)| *keyid == k.keyid())
}

pub fn verify_signature(content: &str, signature: &str) -> Result<bool> {
    let keys = load_keys()?;
    let mut sigs = parse_signatures(signature)?;

    // Build the hashes
    let hash_algos: Vec<HashAlgorithm> = sigs
        .iter()
        .map(|&(ref sig, _, _)| sig.hash_algo())
        .collect();
    let hashes: HashMap<_, _> = openpgp::crypto::hash_file(content.as_bytes(), &hash_algos[..])
        .map_err(squish_internal_err)?
        .into_iter()
        .collect();

    // Apply the keys to the signatures
    for tpk in keys {
        for &mut (_, ref issuer, ref mut issuer_tpko) in sigs.iter_mut() {
            if tpk_has_key(&tpk, issuer) {
                if let Some(issuer_tpk) = issuer_tpko.take() {
                    *issuer_tpko = issuer_tpk.merge(tpk.clone()).ok();
                } else {
                    *issuer_tpko = Some(tpk.clone());
                }
            }
        }
    }

    // Verify the signatures.
    let mut sigs_seen_from_tpk = HashSet::new();
    let mut good = 0;

    'sig_loop: for (mut sig, issuer, tpko) in sigs.into_iter() {
        if let Some(ref tpk) = tpko {
            // Find the right key.
            for (maybe_binding, _, key) in tpk.keys_all() {
                let binding = match maybe_binding {
                    Some(b) => b,
                    None => continue,
                };

                if issuer == key.keyid() {
                    if !binding.key_flags().can_sign() {
                        eprintln!(
                            "Cannot check signature, key has no signing \
                             capability"
                        );
                        continue 'sig_loop;
                    }

                    let mut hash = match hashes.get(&sig.hash_algo()) {
                        Some(h) => h.clone(),
                        None => {
                            eprintln!(
                                "Cannot check signature, hash algorithm \
                                 {} not supported.",
                                sig.hash_algo()
                            );
                            continue 'sig_loop;
                        }
                    };
                    sig.hash(&mut hash);

                    let mut digest = vec![0u8; hash.digest_size()];
                    hash.digest(&mut digest);
                    let hash_algo = sig.hash_algo();
                    sig.set_computed_hash(Some((hash_algo, digest)));

                    match sig.verify(key) {
                        Ok(true) => {
                            if let Some(t) = sig.signature_creation_time() {
                                //if let Some(not_before) = not_before {
                                //    if t < not_before {
                                //        eprintln!(
                                //            "Signature by {} was created before \
                                //             the --not-before date.",
                                //            issuer
                                //        );
                                //        break;
                                //    }
                                //}
                                //
                                //if t > not_after {
                                //    eprintln!(
                                //        "Signature by {} was created after \
                                //         the --not-after date.",
                                //        issuer
                                //    );
                                //    break;
                                //}

                                // check key was valid at sig creation time
                                let binding = tpk
                                    .subkeys()
                                    .find(|s| s.subkey().fingerprint() == key.fingerprint());
                                if let Some(binding) = binding {
                                    if binding.revoked(t) != RevocationStatus::NotAsFarAsWeKnow {
                                        eprintln!(
                                            "Key was revoked when the signature \
                                             was created."
                                        );
                                        break;
                                    }
                                }

                                if tpk.revocation_status_at(t) != RevocationStatus::NotAsFarAsWeKnow
                                {
                                    eprintln!(
                                        "Primary key was revoked when the \
                                         signature was created."
                                    );
                                    break;
                                }
                            } else {
                                eprintln!(
                                    "Signature by {} does not contain \
                                     information about the creation time.",
                                    issuer
                                );
                                break;
                            }

                            if sigs_seen_from_tpk.replace(tpk.fingerprint()).is_some() {
                                eprintln!("Ignoring additional good signature by {}.", issuer);
                                continue;
                            }

                            eprintln!("info: Good signature from {}", tpk.primary().fingerprint());
                            good += 1;
                        }
                        Ok(false) => {
                            Err(squish_internal_err(format!(
                                "Signature by {} is bad.",
                                issuer
                            )))?;
                        }
                        Err(err) => {
                            Err(squish_internal_err(err))?;
                        }
                    }

                    break;
                }
            }
        } else {
            eprintln!("Can't verify signature by {}, missing key.", issuer);
        }
    }

    Ok(good > 0)
}
