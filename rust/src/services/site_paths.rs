pub fn localized_root_path() -> String {
    "/".to_string()
}

pub fn locale_prefix() -> String {
    String::new()
}

pub fn localized_path(path_and_query: &str) -> String {
    if path_and_query.starts_with('/') {
        path_and_query.to_string()
    } else {
        format!("/{}", path_and_query)
    }
}

#[cfg(test)]
mod tests {
    use super::{locale_prefix, localized_path, localized_root_path};

    #[test]
    fn public_paths_have_no_language_prefix() {
        assert_eq!(localized_root_path(), "/");
        assert_eq!(locale_prefix(), "");
        assert_eq!(localized_path("/content/story"), "/content/story");
    }
}
