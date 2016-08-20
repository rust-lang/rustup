extern crate crypt32;
extern crate winapi;

use super::super::CertItem;
use std::ffi::CString;
use std::ptr;
use std::result::Result;
use std::slice::from_raw_parts;

pub struct CertBundle {
    store: winapi::HCERTSTORE,
    ctx_p: winapi::PCCERT_CONTEXT
}

pub struct CertIter {
    bundle: CertBundle
}

impl IntoIterator for CertBundle {
    type Item = CertItem;
    type IntoIter = CertIter;

    fn into_iter(self) -> Self::IntoIter {
        CertIter { bundle: self }
    }
}

impl Iterator for CertIter {
    type Item = CertItem;

    fn next(&mut self) -> Option<CertItem> {
        if self.bundle.ctx_p.is_null() {
            return None;
        }
        unsafe {
            let ctx = *self.bundle.ctx_p;
            let enc_slice = from_raw_parts(
                ctx.pbCertEncoded as *const u8,
                ctx.cbCertEncoded as usize);
            let mut blob = Vec::with_capacity(ctx.cbCertEncoded as usize);
            blob.extend_from_slice(enc_slice);
            self.bundle.ctx_p = crypt32::CertEnumCertificatesInStore(
                self.bundle.store,
                self.bundle.ctx_p);
            Some(CertItem::Blob(blob))
        }
    }
}

impl CertBundle {
    pub fn new() -> Result<CertBundle, ()> {
        unsafe {
            let store = crypt32::CertOpenSystemStoreA(
                0,
                CString::new("Root").unwrap().as_ptr() as winapi::LPCSTR);
            if store.is_null() {
                return Err(());
            }
            let ctx_p = crypt32::CertEnumCertificatesInStore(
                store,
                ptr::null());
            Ok(CertBundle {
                store: store,
                ctx_p: ctx_p
            })
        }
    }
}

impl Drop for CertBundle {
    fn drop(&mut self) {
        unsafe {
            if !self.ctx_p.is_null() {
                crypt32::CertFreeCertificateContext(self.ctx_p);
            }
            crypt32::CertCloseStore(self.store, 0);
        }
    }
}
