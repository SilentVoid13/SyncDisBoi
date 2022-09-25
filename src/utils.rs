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
            format!("{}{}", &a[..a.len()-1], &b.trim_end())
        },
        None => name.to_string(),
    }
}

pub fn clean_quotes(name: &str) -> String {
    name.replace("'", "")
}
