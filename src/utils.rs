pub fn clean_song_name(name: &str) -> String {
    let name = clean_parenthesis(&name, "(feat.");
    let name = clean_parenthesis(&name, "(with");
    name
}

pub fn clean_parenthesis(name: &str, tag: &str) -> String {
    // to what extent are we ready to go to avoid regex...
    match name.find(tag) {
        Some(n) => {
            let (a, b) = name.split_at(n);
            let b: String = b.chars().skip_while(|c| *c != ')').skip(1).collect();
            format!("{}{}", &a.trim_end(), &b)
        },
        None => name.to_string(),
    }
}

pub fn clean_bad_chars_spotify(name: &str) -> String {
    let name = name.replace("'", "");
    let name = name.replace("\"", "");
    let name = name.replace(":", " ");
    name
}
