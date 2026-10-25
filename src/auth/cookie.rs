//! Session cookie shared with the other eigenplay games
//!
//! The JWT is also set as an `HttpOnly` cookie so that sibling sites
//! (e.g. azul.eigenplay.com) can read the session when `AUTH_COOKIE_DOMAIN`
//! names their parent domain (e.g. `.eigenplay.com`).

/// Name of the session cookie
pub const SESSION_COOKIE: &str = "eigenplay_session";

/// Domain of the cookie, from `AUTH_COOKIE_DOMAIN` (unset: current host only)
pub fn cookie_domain() -> Option<String> {
    std::env::var("AUTH_COOKIE_DOMAIN")
        .ok()
        .filter(|domain| !domain.is_empty())
}

/// `Set-Cookie` value holding `token` for `max_age_secs`
pub fn session_cookie(token: &str, max_age_secs: u64, domain: Option<&str>) -> String {
    cookie(token, max_age_secs, domain)
}

/// `Set-Cookie` value removing the session cookie
pub fn cleared_cookie(domain: Option<&str>) -> String {
    cookie("", 0, domain)
}

fn cookie(value: &str, max_age_secs: u64, domain: Option<&str>) -> String {
    let mut cookie = format!(
        "{SESSION_COOKIE}={value}; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age={max_age_secs}"
    );
    if let Some(domain) = domain {
        cookie.push_str("; Domain=");
        cookie.push_str(domain);
    }
    cookie
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_cookie_holds_the_token_for_its_lifetime() {
        let cookie = session_cookie("abc.def.ghi", 86400, None);

        assert!(cookie.starts_with("eigenplay_session=abc.def.ghi;"));
        assert!(cookie.contains("Max-Age=86400"));
    }

    #[test]
    fn session_cookie_is_hidden_from_scripts_and_other_sites() {
        let cookie = session_cookie("t", 60, None);

        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("Secure"));
        assert!(cookie.contains("SameSite=Lax"));
    }

    #[test]
    fn session_cookie_reaches_sibling_sites_of_its_domain() {
        let cookie = session_cookie("t", 60, Some(".eigenplay.com"));

        assert!(cookie.ends_with("; Domain=.eigenplay.com"));
    }

    #[test]
    fn session_cookie_without_domain_stays_on_its_host() {
        assert!(!session_cookie("t", 60, None).contains("Domain="));
    }

    #[test]
    fn cleared_cookie_expires_at_once_on_the_same_domain() {
        let cookie = cleared_cookie(Some(".eigenplay.com"));

        assert!(cookie.starts_with("eigenplay_session=;"));
        assert!(cookie.contains("Max-Age=0"));
        assert!(cookie.contains("Domain=.eigenplay.com"));
    }
}
