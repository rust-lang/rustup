extern crate security_framework as sf;

use super::super::CertItem;
use self::sf::item::{ItemClass, ItemSearchOptions, Reference, SearchResult};
use self::sf::keychain::SecKeychain;
use self::sf::os::macos::keychain::SecKeychainExt;
use std::i32;
use std::result::Result;

pub struct CertBundle {
    rv: Vec<SearchResult>
}

pub struct CertIter {
    it: Box<Iterator<Item=SearchResult>>
}

impl IntoIterator for CertBundle {
    type Item = CertItem;
    type IntoIter = CertIter;

    fn into_iter(self) -> Self::IntoIter {
        CertIter { it: Box::new(self.rv.into_iter()) }
    }
}

impl Iterator for CertIter {
    type Item = CertItem;

    fn next(&mut self) -> Option<CertItem> {
        if let Some(res) = self.it.next() {
            if let Some(ref rref) = res.reference {
                match rref {
                    &Reference::Certificate(ref cert) => return Some(CertItem::Blob(cert.to_der())),
                    _ => ()
                }
            }
            return self.next();
        }
        None
    }
}

impl CertBundle {
    pub fn new() -> Result<CertBundle, ()> {
        let root_kc = try!(SecKeychain::open("/System/Library/Keychains/SystemRootCertificates.keychain").map_err(|_| ()));
        let chains = [ root_kc ];
        let mut opts = ItemSearchOptions::new();
        let opts = opts.keychains(&chains).class(ItemClass::Certificate).load_refs(true).limit(i32::MAX as i64);
        let rv = try!(opts.search().map_err(|_| ()));
        Ok(CertBundle { rv: rv })
    }
}
