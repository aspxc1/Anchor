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

pub fn validate_domains(domains: Vec<String>) -> Result<Vec<String>, String> {
    if domains.len() > 100 {
        return Err("Save up to 100 allowed website patterns.".into());
    }
    let mut output = Vec::with_capacity(domains.len());
    for pattern in domains {
        let pattern = pattern.trim();
        let (host, path) = pattern
            .split_once('/')
            .map_or((pattern, None), |(host, path)| (host, Some(path)));
        let host = host.to_ascii_lowercase();
        let (wildcard, name) = host
            .strip_prefix("*.")
            .map_or(("", host.as_str()), |name| ("*.", name));
        let name = name
            .trim_end_matches('.')
            .strip_prefix("www.")
            .unwrap_or_else(|| name.trim_end_matches('.'));
        if pattern.len() > 500
            || !hostname_valid(name)
            || pattern.chars().any(|character| {
                character.is_whitespace() || character.is_control() || character == '\\'
            })
        {
            return Err(format!("Invalid website pattern {pattern:?}. Try docs.rs, *.mit.edu, or github.com/myorg/*."));
        }
        let mut normalized = format!("{wildcard}{name}");
        if let Some(path) = path {
            normalized.push('/');
            normalized.push_str(path);
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
        assert!(validate_domains(vec!["https://example.com".into()]).is_err());
    }
}
