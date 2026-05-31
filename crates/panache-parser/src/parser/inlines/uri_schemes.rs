//! Pandoc's URI scheme allowlist (IANA + a few unofficial ones).
//!
//! Mirrors `pandoc/src/Text/Pandoc/URI.hs`. Both the bare-URI inline parser
//! and the pandoc-native projector validate scheme strings against this
//! table so prose words ending in `:` (e.g. `Note:`, `TODO:`) are not
//! silently rewritten into bogus autolinks (see issues #197, #336).

/// Returns `true` if `scheme` (ASCII, case-insensitive) is a known URI
/// scheme. The lookup is case-insensitive and performed via binary
/// search; callers should pass just the scheme part (no trailing `:`).
pub fn is_known_scheme(scheme: &str) -> bool {
    if scheme.is_empty() || !scheme.is_ascii() {
        return false;
    }
    let lower = scheme.to_ascii_lowercase();
    PANDOC_KNOWN_SCHEMES.binary_search(&lower.as_str()).is_ok()
}

/// Sorted scheme list for `binary_search`. Mirrors
/// `pandoc/src/Text/Pandoc/URI.hs`'s `schemes` set.
#[rustfmt::skip]
const PANDOC_KNOWN_SCHEMES: &[&str] = &[
    "aaa", "aaas", "about", "acap", "acct", "acr",
    "adiumxtra", "afp", "afs", "aim", "appdata", "apt",
    "attachment", "aw", "barion", "beshare", "bitcoin", "blob",
    "bolo", "browserext", "callto", "cap", "chrome", "chrome-extension",
    "cid", "coap", "coaps", "com-eventbrite-attendee", "content", "crid",
    "cvs", "data", "dav", "dict", "dis", "dlna-playcontainer",
    "dlna-playsingle", "dns", "dntp", "doi", "dtn", "dvb",
    "ed2k", "example", "facetime", "fax", "feed", "feedready",
    "file", "filesystem", "finger", "fish", "ftp", "gemini",
    "geo", "gg", "git", "gizmoproject", "go", "gopher",
    "graph", "gtalk", "h323", "ham", "hcp", "http",
    "https", "hxxp", "hxxps", "hydrazone", "iax", "icap",
    "icon", "im", "imap", "info", "iotdisco", "ipn",
    "ipp", "ipps", "irc", "irc6", "ircs", "iris",
    "iris.beep", "iris.lwz", "iris.xpc", "iris.xpcs", "isbn", "isostore",
    "itms", "jabber", "jar", "javascript", "jms", "keyparc",
    "lastfm", "ldap", "ldaps", "lvlt", "magnet", "mailserver",
    "mailto", "maps", "market", "message", "mid", "mms",
    "modem", "mongodb", "moz", "ms-access", "ms-browser-extension", "ms-drive-to",
    "ms-enrollment", "ms-excel", "ms-gamebarservices", "ms-getoffice", "ms-help", "ms-infopath",
    "ms-media-stream-id", "ms-officeapp", "ms-powerpoint", "ms-project", "ms-publisher", "ms-search-repair",
    "ms-secondary-screen-controller", "ms-secondary-screen-setup", "ms-settings", "ms-settings-airplanemode", "ms-settings-bluetooth", "ms-settings-camera",
    "ms-settings-cellular", "ms-settings-cloudstorage", "ms-settings-connectabledevices", "ms-settings-displays-topology", "ms-settings-emailandaccounts", "ms-settings-language",
    "ms-settings-location", "ms-settings-lock", "ms-settings-nfctransactions", "ms-settings-notifications", "ms-settings-power", "ms-settings-privacy",
    "ms-settings-proximity", "ms-settings-screenrotation", "ms-settings-wifi", "ms-settings-workplace", "ms-spd", "ms-sttoverlay",
    "ms-transit-to", "ms-virtualtouchpad", "ms-visio", "ms-walk-to", "ms-whiteboard", "ms-whiteboard-cmd",
    "ms-word", "msnim", "msrp", "msrps", "mtqp", "mumble",
    "mupdate", "mvn", "news", "nfs", "ni", "nih",
    "nntp", "notes", "ocf", "oid", "onenote", "onenote-cmd",
    "opaquelocktoken", "pack", "palm", "paparazzi", "pkcs11", "platform",
    "pmid", "pop", "pres", "prospero", "proxy", "psyc",
    "pwid", "qb", "query", "redis", "rediss", "reload",
    "res", "resource", "rmi", "rsync", "rtmfp", "rtmp",
    "rtsp", "rtsps", "rtspu", "secondlife", "service", "session",
    "sftp", "sgn", "shttp", "sieve", "sip", "sips",
    "skype", "smb", "sms", "smtp", "snews", "snmp",
    "soap.beep", "soap.beeps", "soldat", "spotify", "ssh", "steam",
    "stun", "stuns", "submit", "svn", "tag", "teamspeak",
    "tel", "teliaeid", "telnet", "tftp", "things", "thismessage",
    "tip", "tn3270", "tool", "turn", "turns", "tv",
    "udp", "unreal", "urn", "ut2004", "v-event", "vemmi",
    "ventrilo", "videotex", "view-source", "vnc", "wais", "webcal",
    "wpid", "ws", "wss", "wtai", "wyciwyg", "xcon",
    "xcon-userid", "xfire", "xmlrpc.beep", "xmlrpc.beeps", "xmpp", "xri",
    "ymsgr", "z39.50", "z39.50r", "z39.50s",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_is_sorted_for_binary_search() {
        for window in PANDOC_KNOWN_SCHEMES.windows(2) {
            assert!(window[0] < window[1], "out of order: {:?}", window);
        }
    }

    #[test]
    fn known_schemes() {
        assert!(is_known_scheme("http"));
        assert!(is_known_scheme("https"));
        assert!(is_known_scheme("ftp"));
        assert!(is_known_scheme("mailto"));
        assert!(is_known_scheme("tel"));
        assert!(is_known_scheme("urn"));
    }

    #[test]
    fn case_insensitive() {
        assert!(is_known_scheme("HTTP"));
        assert!(is_known_scheme("MailTo"));
    }

    #[test]
    fn unknown_schemes_rejected() {
        assert!(!is_known_scheme("note"));
        assert!(!is_known_scheme("todo"));
        assert!(!is_known_scheme("a"));
        assert!(!is_known_scheme(""));
    }
}
