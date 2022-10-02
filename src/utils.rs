pub fn clean_enclosure(name: &str, start_tag: &str, end_tag: &str) -> String {
    let mut res = vec![];
    for split in name.split(start_tag) {
        match split.split_once(end_tag) {
            Some((_, a)) => res.push(a),
            None => res.push(split.trim_end())
        };
    }
    res.join("").trim_end().to_string()
}

pub fn generic_name_clean(name: &str) -> String {
    let name = name.to_lowercase();
    let name = name.replace("'", "");
    let name = name.replace("\"", "");
    let name = name.replace(":", " ");
    let name = clean_enclosure(&name, "(", ")");
    let name = clean_enclosure(&name, "[", "]");
    let name = name.split(" - ").next().unwrap().to_string();
    let name = name.replace("-", "");
    name.trim_end().to_string()
}
