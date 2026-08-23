#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Intent {
    Organize { target: String },
    Search { query: String },
    FindDuplicates,
    FindLarge,
    Rename { target: String },
    Move { target: String },
    Undo,
    Help,
    Unknown(String),
}

fn extract_target(original: &str, keywords: &[&str]) -> String {
    let trimmed = original.trim();
    let lower = trimmed.to_lowercase();
    for kw in keywords {
        if lower.starts_with(kw) {
            let rest = &trimmed[kw.len()..];
            // strip leading whitespace/punctuation after keyword
            let rest = rest.trim_start_matches([' ', '\t', ':', '-']);
            return rest.trim().to_string();
        }
    }
    String::new()
}

pub fn parse_intent(text: &str) -> Intent {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Intent::Unknown(trimmed.to_string());
    }
    let lower = trimmed.to_lowercase();

    // exact simple commands
    if lower == "undo" || lower.starts_with("undo ") || lower.starts_with("undo\t") {
        return Intent::Undo;
    }
    if lower == "help" || lower.starts_with("help ") || lower == "?" || lower == "?" {
        return Intent::Help;
    }
    // duplicate / dupes anywhere -> FindDuplicates
    if lower.contains("duplicate") || lower.contains("dupes") || lower.contains("dupe") {
        return Intent::FindDuplicates;
    }
    if lower.contains("large") || lower.contains("big") {
        return Intent::FindLarge;
    }
    if lower.starts_with("organize") || lower.starts_with("clean") {
        let target = extract_target(trimmed, &["organize", "clean"]);
        return Intent::Organize { target };
    }
    if lower.starts_with("rename") {
        let target = extract_target(trimmed, &["rename"]);
        return Intent::Rename { target };
    }
    if lower.starts_with("move") {
        let target = extract_target(trimmed, &["move"]);
        return Intent::Move { target };
    }
    if lower.starts_with("find") || lower.starts_with("search") {
        let query = extract_target(trimmed, &["find", "search"]);
        let q = if query.is_empty() {
            // if no remainder, use trimmed as query or empty
            // for "find" alone, keep empty; for "find cats" return cats
            query
        } else {
            query
        };
        // if find/search with no args, treat as Search with original text without prefix stripped?
        // keep behavior: empty query -> Unknown? but spec says Search{query:String}
        return Intent::Search { query: q };
    }

    Intent::Unknown(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn organize_prefix() {
        assert!(
            matches!(parse_intent("organize my downloads"), Intent::Organize { target } if target == "my downloads")
        );
        assert!(matches!(
            parse_intent("Organize /tmp"),
            Intent::Organize { .. }
        ));
        assert!(
            matches!(parse_intent("clean my folder"), Intent::Organize { target } if target == "my folder")
        );
        assert!(matches!(parse_intent("CLEAN"), Intent::Organize { target } if target.is_empty()));
    }

    #[test]
    fn search_prefix() {
        assert!(matches!(parse_intent("find cats"), Intent::Search { query } if query == "cats"));
        assert!(
            matches!(parse_intent("search vacation photos"), Intent::Search { query } if query == "vacation photos")
        );
        assert!(
            matches!(parse_intent("Find my report"), Intent::Search { query } if query == "my report")
        );
        assert!(matches!(parse_intent("SEARCH"), Intent::Search { query } if query.is_empty()));
    }

    #[test]
    fn find_duplicates() {
        assert_eq!(parse_intent("find duplicates"), Intent::FindDuplicates);
        assert_eq!(parse_intent("duplicate files"), Intent::FindDuplicates);
        assert_eq!(parse_intent("show dupes"), Intent::FindDuplicates);
        assert_eq!(parse_intent("FIND DUPE"), Intent::FindDuplicates);
    }

    #[test]
    fn find_large() {
        assert_eq!(parse_intent("find large files"), Intent::FindLarge);
        assert_eq!(parse_intent("big files"), Intent::FindLarge);
        assert_eq!(parse_intent("show large"), Intent::FindLarge);
        assert_eq!(parse_intent("BIG"), Intent::FindLarge);
    }

    #[test]
    fn rename_move() {
        assert!(
            matches!(parse_intent("rename photo.jpg"), Intent::Rename { target } if target == "photo.jpg")
        );
        assert!(
            matches!(parse_intent("move /tmp/a to /tmp/b"), Intent::Move { target } if target == "/tmp/a to /tmp/b")
        );
        assert!(matches!(
            parse_intent("Rename MyFile"),
            Intent::Rename { .. }
        ));
        assert!(matches!(parse_intent("Move docs"), Intent::Move { .. }));
    }

    #[test]
    fn undo() {
        assert_eq!(parse_intent("undo"), Intent::Undo);
        assert_eq!(parse_intent("Undo last"), Intent::Undo);
        assert_eq!(parse_intent("UNDO"), Intent::Undo);
    }

    #[test]
    fn help() {
        assert_eq!(parse_intent("help"), Intent::Help);
        assert_eq!(parse_intent("Help me"), Intent::Help);
        assert_eq!(parse_intent("?"), Intent::Help);
    }

    #[test]
    fn unknown() {
        assert!(matches!(parse_intent("hello world"), Intent::Unknown(s) if s == "hello world"));
        assert!(matches!(parse_intent("foobar"), Intent::Unknown(_)));
        assert!(matches!(parse_intent(""), Intent::Unknown(s) if s.is_empty()));
    }

    #[test]
    fn case_insensitive() {
        assert!(matches!(
            parse_intent("ORGANIZE downloads"),
            Intent::Organize { .. }
        ));
        assert!(matches!(parse_intent("SEARCH cats"), Intent::Search { .. }));
        assert_eq!(parse_intent("DuPlIcAtE"), Intent::FindDuplicates);
        assert_eq!(parse_intent("LaRgE"), Intent::FindLarge);
    }
}
