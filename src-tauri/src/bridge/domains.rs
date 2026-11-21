pub fn hostname_valid(hostname: &str) -> bool {
    !hostname.is_empty()
        && hostname.len() <= 253
        && hostname.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|character| character.is_ascii_alphanumeric() || character == b'-')
        })
}

fn strip_http_scheme(value: &str) -> &str {
    if value.len() >= 8 && value[..8].eq_ignore_ascii_case("https://") {
        &value[8..]
    } else if value.len() >= 7 && value[..7].eq_ignore_ascii_case("http://") {
        &value[7..]
    } else {
        value
    }
}

pub fn validate_domains(domains: Vec<String>) -> Result<Vec<String>, String> {
    if domains.len() > 100 {
        return Err("Save up to 100 allowed website patterns.".into());
    }
    let mut output = Vec::with_capacity(domains.len());
    for pattern in domains {
        let raw = pattern.trim();
        let without_scheme = strip_http_scheme(raw);
        let without_fragment = without_scheme
            .split('#')
            .next()
            .unwrap_or(without_scheme);
        let (host_part, suffix) = match without_fragment.find(['/', '?']) {
            Some(index) => (&without_fragment[..index], Some(&without_fragment[index..])),
            None => (without_fragment, None),
        };
        let host_without_port = match host_part.rsplit_once(':') {
            Some((host, port)) if !host.is_empty() && !port.is_empty() && port.parse::<u16>().is_ok() => host,
            _ => host_part,
        };
        let host = host_without_port.to_ascii_lowercase();
        let (wildcard, name) = host
            .strip_prefix("*.")
            .map_or(("", host.as_str()), |name| ("*.", name));
        let name = name
            .trim_end_matches('.')
            .strip_prefix("www.")
            .unwrap_or_else(|| name.trim_end_matches('.'));
        if raw.len() > 500
            || !hostname_valid(name)
            || raw.chars().any(|character| {
                character.is_whitespace() || character.is_control() || character == '\\'
            })
        {
            return Err(format!("Invalid website pattern {raw:?}. Try docs.rs, *.mit.edu, or github.com/myorg/*."));
        }
        let mut normalized = format!("{wildcard}{name}");
        if let Some(suffix) = suffix {
            if suffix != "/" && suffix != "/*" {
                normalized.push_str(suffix);
            }
        }
        if !output.contains(&normalized) {
            output.push(normalized);
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domains_are_normalized_and_paths_are_preserved() {
        let domains = vec![
            "WWW.GitHub.com/Org/*".into(),
            "github.com/Org/*".into(),
            "*.MIT.EDU".into(),
        ];
        assert_eq!(
            validate_domains(domains).unwrap(),
            ["github.com/Org/*", "*.mit.edu"]
        );
    }

    #[test]
    fn http_and_https_urls_are_normalized() {
        let domains = vec![
            "https://lms.eng.asu.edu.eg/*".into(),
            "https://lms.eng.asu.edu.eg/".into(),
            "HTTP://WWW.Example.com:8080/docs/page#intro".into(),
            "https://github.com/myorg/*".into(),
            "https://*.mit.edu/*".into(),
        ];
        assert_eq!(
            validate_domains(domains).unwrap(),
            [
                "lms.eng.asu.edu.eg",
                "example.com/docs/page",
                "github.com/myorg/*",
                "*.mit.edu",
            ]
        );
        assert!(validate_domains(vec!["ftp://example.com".into()]).is_err());
        assert!(validate_domains(vec!["https://".into()]).is_err());
        assert!(validate_domains(vec!["https://user:pass@example.com".into()]).is_err());
    }
}
