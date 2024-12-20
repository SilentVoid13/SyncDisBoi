use regex::Regex;

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
    for (a, b) in replaces.iter() {
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
