use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};

/// Encode query values the way Foundation's `urlQueryAllowed` minus `&=+?#` does.
/// Spaces become `%20`, not `+`.
const QUERY_VALUE: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'&')
    .add(b'+')
    .add(b',')
    .add(b'/')
    .add(b':')
    .add(b';')
    .add(b'<')
    .add(b'=')
    .add(b'>')
    .add(b'?')
    .add(b'@')
    .add(b'[')
    .add(b'\\')
    .add(b']')
    .add(b'^')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}');

/// Path segments: unreserved plus sub-delims and `@`, never `/`.
const PATH_SEGMENT: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'`')
    .add(b'{')
    .add(b'}')
    .add(b'/');

/// Path + query the server reads with PHP `getRequestUri()`, so both sides sign the same bytes.
pub fn target(path: &str, query: &[(&str, Option<&str>)]) -> String {
    let encoded_path: String = path
        .split('/')
        .map(|segment| utf8_percent_encode(segment, PATH_SEGMENT).to_string())
        .collect::<Vec<_>>()
        .join("/");

    let mut pairs: Vec<(&str, &str)> = query
        .iter()
        .filter_map(|(k, v)| v.map(|value| (*k, value)))
        .collect();
    pairs.sort_by(|a, b| a.0.cmp(b.0));

    let mut uri = format!("/api/v1/{encoded_path}");
    if !pairs.is_empty() {
        uri.push('?');
        uri.push_str(
            &pairs
                .into_iter()
                .map(|(k, v)| format!("{k}={}", utf8_percent_encode(v, QUERY_VALUE)))
                .collect::<Vec<_>>()
                .join("&"),
        );
    }
    uri
}

#[cfg(test)]
mod tests {
    use super::target;

    #[test]
    fn keeps_path_and_sorted_encoded_query() {
        assert_eq!(
            target(
                "remotes/@nuno/messages",
                &[("reactions_after", Some("3")), ("after", Some("10"))]
            ),
            "/api/v1/remotes/@nuno/messages?after=10&reactions_after=3"
        );
        assert_eq!(
            target("remotes", &[("q", Some("a b&c")), ("tab", None)]),
            "/api/v1/remotes?q=a%20b%26c"
        );
    }
}
