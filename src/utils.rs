use color_eyre::Result;
use regex::Regex;
use serde::de::DeserializeOwned;
use tracing::error;

use crate::{ConfigArgs, music_api::Song};

pub fn clean_enclosure(name: &str, start_tag: char, end_tag: char) -> String {
    if name.contains(start_tag) {
        let mut res = vec![];
        let mut chars = name.chars().peekable();
        while chars.peek().is_some() {
            let s: String = chars.by_ref().take_while(|c| *c != start_tag).collect();
            res.push(s);

            let mut opened = 1;
            while opened > 0 {
                let _ = chars
                    .by_ref()
                    .take_while(|c| {
                        if *c == start_tag {
                            opened += 1;
                        }
                        *c != end_tag
                    })
                    .count();
                opened -= 1;
            }
        }
        res.push(chars.collect());
        return res.join("").trim_end().to_string();
    }
    name.to_string()
}

pub fn generic_name_clean(name: &str) -> String {
    let mut name = name.to_lowercase();
    let replaces = [
        ("'", ""),
        ("\"", ""),
        (":", " "),
        ("%", ""),
        ("é", "e"),
        ("è", "e"),
        ("à", "a"),
    ];
    for (a, b) in replaces {
        name = name.replace(a, b);
    }
    let part_re = Regex::new(r"\((part (?:[a-zA-Z]+|[0-9]+))\)").unwrap();
    if part_re.is_match(&name) {
        name = part_re.replace_all(&name, "$1").to_string();
    }
    let name = clean_enclosure(&name, '(', ')');
    let name = clean_enclosure(&name, '[', ']');
    name.trim_end().to_string()
}

#[inline]
pub fn clean_isrc(isrc: Option<String>) -> Option<String> {
    if let Some(isrc) = isrc {
        let isrc = isrc.trim().to_uppercase().replace('-', "");
        if isrc.len() != 12 {
            error!("invalid ISRC code found: {}, ignoring it", isrc);
            return None;
        }
        return Some(isrc);
    }
    None
}

pub fn dedup_songs(songs: &mut Vec<Song>) -> bool {
    let mut seen = std::collections::HashSet::new();
    let mut dups = false;
    let mut i = 0;
    let mut len = songs.len();
    while i < len {
        if seen.insert(songs[i].id.clone()) {
            i += 1;
        } else {
            songs.remove(i);
            dups = true;
            len -= 1;
        }
    }
    dups
}

pub async fn debug_response_json<T>(
    config: &ConfigArgs,
    res: reqwest::Response,
    platform: &str,
) -> Result<T>
where
    T: DeserializeOwned,
{
    const DEBUG_FOLDER: &str = "debug";

    let res = if config.debug {
        let text = res.text().await?;
        std::fs::write(
            format!("{}/{}_last_res.json", DEBUG_FOLDER, platform),
            &text,
        )?;
        serde_json::from_str(&text).inspect_err(|_| {
            let _ = std::fs::write(format!("debug/{}_last_error.json", platform), &text);
        })?
    } else {
        res.json().await?
    };
    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_enclosure() {
        let name =
            "POP/STARS (feat. (G)I-DLE, Madison Beer, Jaira Burns & League ((A)) of Legends) test";
        let res = clean_enclosure(name, '(', ')');
        assert_eq!(res, "POP/STARS  test");

        let name = "test (feat. test) test (feat. test2)";
        let res = clean_enclosure(name, '(', ')');
        assert_eq!(res, "test  test");
    }
}
