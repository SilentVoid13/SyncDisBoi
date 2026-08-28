use sync_dis_boi::ConfigArgs;
use sync_dis_boi::export::export;
use sync_dis_boi::import::import;
use sync_dis_boi::music_api::{MusicApiType, Playlist};

use crate::common::mock::{MockApi, block_on};
use crate::common::util::temp_dir;

fn config() -> ConfigArgs {
    ConfigArgs {
        debug: false,
        like_all: false,
        sync_likes: false,
        diff_country: false,
        proxy: None,
    }
}

#[test]
fn export_writes_every_readable_playlist_with_its_songs() {
    let dir = temp_dir("export");
    let src = MockApi::new(MusicApiType::Tidal);
    src.seed_playlist("A", vec![src.catalog_song(0), src.catalog_song(1)]);
    src.seed_playlist("B", vec![src.catalog_song(2)]);
    let broken = src.seed_playlist("Broken", vec![src.catalog_song(3)]);
    src.make_unreadable(&broken);

    for minify in [false, true] {
        let out = dir.join(format!("export-{minify}.json"));
        block_on(export(src.boxed(), &out, minify)).unwrap();

        let text = std::fs::read_to_string(&out).unwrap();
        assert_eq!(text.contains('\n'), !minify);
        let exported: Vec<Playlist> = serde_json::from_str(&text).unwrap();
        let summary: Vec<(&str, usize)> = exported
            .iter()
            .map(|p| (p.name.as_str(), p.songs.len()))
            .collect();
        assert_eq!(summary, [("A", 2), ("B", 1), ("Broken", 0)]);
        let a = &exported[0].songs[0];
        assert_eq!(a.id, src.catalog_song(0).id);
        assert_eq!(a.isrc, src.catalog_song(0).isrc);
    }
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn import_recreates_an_export_on_another_platform() {
    let dir = temp_dir("import");
    let src = MockApi::new(MusicApiType::Spotify);
    src.seed_playlist("A", vec![src.catalog_song(0), src.catalog_song(4)]);
    src.seed_playlist("B", vec![src.catalog_song(2)]);
    let file = dir.join("export.json");
    block_on(export(src.boxed(), &file, false)).unwrap();

    for kind in [
        MusicApiType::Tidal,
        MusicApiType::YtMusic,
        MusicApiType::ListenBrainz,
    ] {
        let dst = MockApi::new(kind.clone());
        block_on(import(&file, dst.boxed(), config())).unwrap();
        // importing twice must not duplicate anything
        block_on(import(&file, dst.boxed(), config())).unwrap();

        assert_eq!(dst.playlist_count(), 2, "{kind:?}");
        let a: Vec<String> = dst
            .playlist("A")
            .unwrap()
            .songs
            .iter()
            .map(|s| s.id.clone())
            .collect();
        assert_eq!(
            a,
            [dst.catalog_song(0).id, dst.catalog_song(4).id],
            "{kind:?}"
        );
        assert_eq!(dst.playlist("B").unwrap().songs.len(), 1, "{kind:?}");
    }
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn import_of_a_malformed_file_fails_without_touching_the_platform() {
    let dir = temp_dir("import-bad");
    let file = dir.join("bad.json");
    std::fs::write(&file, "{ not json").unwrap();
    let dst = MockApi::new(MusicApiType::Tidal);
    assert!(block_on(import(&file, dst.boxed(), config())).is_err());
    assert_eq!(dst.playlist_count(), 0);
    std::fs::remove_dir_all(&dir).ok();
}
