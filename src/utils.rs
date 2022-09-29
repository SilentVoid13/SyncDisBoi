pub fn clean_song_name(name: &str) -> String {
    let name = clean_enclosure(&name, "(", ")");
    let name = clean_enclosure(&name, "[", "]");
    //let name = clean_enclosure(&name, "(feat.");
    //let name = clean_enclosure(&name, "(with");
    name
}

pub fn clean_enclosure(name: &str, start_tag: &str, end_tag: &str) -> String {
    let mut res = vec![];
    for split in name.split(start_tag) {
        match split.split_once(end_tag) {
            Some((_, a)) => res.push(a),
            None => res.push(split.trim_end())
        };
    }
    res.join("")
}

pub fn clean_bad_chars_spotify(name: &str) -> String {
    let name = name.replace("'", "");
    let name = name.replace("\"", "");
    let name = name.replace(":", " ");
    name
}
