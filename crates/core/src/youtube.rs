use crate::watch_party::valid_video_id;

const HOSTS: [&str; 4] = [
    "youtube.com",
    "www.youtube.com",
    "m.youtube.com",
    "music.youtube.com",
];

pub fn first_video_id(text: &str) -> Option<String> {
    text.split_whitespace()
        .find_map(|token| video_id(token.trim_matches(is_trailing_punctuation)))
}

pub fn video_id(value: &str) -> Option<String> {
    let without_scheme = value
        .strip_prefix("https://")
        .or_else(|| value.strip_prefix("http://"))
        .unwrap_or(value);
    if let Some(rest) = without_scheme.strip_prefix("youtu.be/") {
        return take_id(rest);
    }
    if let Some(rest) = without_scheme.strip_prefix("www.youtube-nocookie.com/") {
        return path_id(rest);
    }
    for host in HOSTS {
        if let Some(rest) = without_scheme.strip_prefix(host) {
            let rest = rest.trim_start_matches('/');
            if let Some(query) = rest.strip_prefix("watch") {
                return query_id(query);
            }
            return path_id(rest);
        }
    }
    None
}

fn path_id(rest: &str) -> Option<String> {
    let mut parts = rest.split('/');
    let first = parts.next()?;
    if matches!(first, "shorts" | "embed" | "live" | "v") {
        return parts.next().and_then(take_id);
    }
    None
}

fn query_id(query: &str) -> Option<String> {
    let query = query.split('#').next().unwrap_or(query);
    let query = query.strip_prefix('?').unwrap_or(query);
    for pair in query.split('&') {
        if let Some(value) = pair.strip_prefix("v=") {
            return take_id(value);
        }
    }
    None
}

fn take_id(value: &str) -> Option<String> {
    let value = value.split(['?', '#', '&', '/']).next().unwrap_or_default();
    valid_video_id(value).then(|| value.to_owned())
}

fn is_trailing_punctuation(character: char) -> bool {
    matches!(
        character,
        ',' | '.' | ';' | ':' | ')' | ']' | '}' | '"' | '\''
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_supported_youtube_urls() {
        for value in [
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            "https://youtube.com/watch?v=dQw4w9WgXcQ&t=10",
            "https://youtu.be/dQw4w9WgXcQ?t=5",
            "https://www.youtube.com/shorts/dQw4w9WgXcQ",
            "https://music.youtube.com/watch?v=dQw4w9WgXcQ",
            "https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ",
        ] {
            assert_eq!(video_id(value).as_deref(), Some("dQw4w9WgXcQ"), "{value}");
        }
    }

    #[test]
    fn finds_first_video_in_text_and_rejects_other_hosts() {
        assert_eq!(
            first_video_id("watch this https://youtu.be/dQw4w9WgXcQ, ok?").as_deref(),
            Some("dQw4w9WgXcQ")
        );
        assert!(video_id("https://example.com/watch?v=dQw4w9WgXcQ").is_none());
        assert!(video_id("https://youtube.com/watch?v=bad id").is_none());
    }
}
