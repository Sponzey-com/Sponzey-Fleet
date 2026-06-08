const REDACTED: &str = "[REDACTED]";

pub fn redact_secret(value: &str) -> String {
    let mut output = value.to_owned();
    for marker in ["token=", "password=", "secret=", "private_key="] {
        output = redact_marker(&output, marker);
    }
    output
}

fn redact_marker(value: &str, marker: &str) -> String {
    let mut remaining = value;
    let mut output = String::new();

    while let Some(index) = remaining.find(marker) {
        let (before, after_before) = remaining.split_at(index);
        output.push_str(before);
        output.push_str(marker);
        output.push_str(REDACTED);

        let secret_start = marker.len();
        let after_marker = &after_before[secret_start..];
        let secret_end = after_marker
            .find(|character: char| character.is_whitespace() || character == '&')
            .unwrap_or(after_marker.len());
        remaining = &after_marker[secret_end..];
    }

    output.push_str(remaining);
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_token_like_values() {
        assert_eq!(
            redact_secret("agent enroll token=abc123 env=dev"),
            "agent enroll token=[REDACTED] env=dev"
        );
    }

    #[test]
    fn redacts_multiple_secret_markers() {
        assert_eq!(
            redact_secret("password=p1 secret=s1"),
            "password=[REDACTED] secret=[REDACTED]"
        );
    }
}
