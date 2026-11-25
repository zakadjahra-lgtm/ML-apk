//! Offline accounts: just a player name and a stable UUID derived from it.
//! Offline accounts work for single player, LAN and offline-mode servers.
//! Online-mode servers require a real Microsoft account.

use md5::{Digest, Md5};

/// Same algorithm the vanilla game uses for offline players:
/// UUID v3 of "OfflinePlayer:<name>".
pub fn offline_uuid(name: &str) -> String {
    let mut h = Md5::new();
    h.update(format!("OfflinePlayer:{name}").as_bytes());
    let mut b: [u8; 16] = h.finalize().into();
    b[6] = (b[6] & 0x0f) | 0x30; // version 3
    b[8] = (b[8] & 0x3f) | 0x80; // RFC 4122 variant
    let s = hex::encode(b);
    format!("{}-{}-{}-{}-{}", &s[0..8], &s[8..12], &s[12..16], &s[16..20], &s[20..32])
}

/// Minecraft names: 3-16 chars, letters, digits and underscore.
pub fn valid_username(name: &str) -> bool {
    (3..=16).contains(&name.len()) && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuid_is_stable_and_well_formed() {
        let a = offline_uuid("Steve");
        assert_eq!(a, offline_uuid("Steve"));
        assert_ne!(a, offline_uuid("Alex"));
        assert_eq!(a.len(), 36);
        assert_eq!(&a[14..15], "3");
    }

    #[test]
    fn username_validation() {
        assert!(valid_username("Steve_01"));
        assert!(!valid_username("ab"));
        assert!(!valid_username("has space"));
        assert!(!valid_username("this_name_is_way_too_long"));
    }
}
