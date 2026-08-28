//! `synchronize` / `synchronize_playlists` / `synchronize_likes` behaviour,
//! against mock platforms whose state can be inspected afterwards.

use sync_dis_boi::ConfigArgs;
use sync_dis_boi::music_api::{MusicApi, MusicApiType, Playlist, Song};
use sync_dis_boi::sync::{
    SKIPPED_PLAYLISTS, synchronize, synchronize_likes, synchronize_playlists,
};

use crate::common::fixtures::{FIXTURES, albumless_song, unknown_song};
use crate::common::mock::{MockApi, block_on};

const SRC: MusicApiType = MusicApiType::Tidal;
const DST: MusicApiType = MusicApiType::Spotify;

fn config() -> ConfigArgs {
    ConfigArgs {
        debug: false,
        like_all: false,
        sync_likes: false,
        diff_country: false,
        proxy: None,
    }
}

fn source_songs(indices: &[usize]) -> Vec<Song> {
    indices.iter().map(|&i| FIXTURES[i].song(SRC)).collect()
}

fn playlist(name: &str, songs: Vec<Song>) -> Playlist {
    Playlist {
        id: String::new(),
        name: name.to_string(),
        songs,
    }
}

fn ids(songs: &[Song]) -> Vec<String> {
    songs.iter().map(|s| s.id.clone()).collect()
}

fn catalog_ids(api: &MockApi, indices: &[usize]) -> Vec<String> {
    indices.iter().map(|&i| api.catalog_song(i).id).collect()
}

fn sync_into(dst: &MockApi, playlists: Vec<Playlist>, config: &ConfigArgs) {
    block_on(synchronize_playlists(playlists, &dst.boxed(), config)).unwrap();
}

#[test]
fn creates_the_destination_playlist_with_matched_songs() {
    let dst = MockApi::new(DST);
    sync_into(
        &dst,
        vec![playlist("Mix", source_songs(&[0, 1, 2]))],
        &config(),
    );

    let synced = dst.playlist("Mix").expect("playlist was not created");
    assert_eq!(ids(&synced.songs), catalog_ids(&dst, &[0, 1, 2]));
    assert_eq!(dst.playlist_count(), 1);
}

#[test]
fn reuses_an_existing_playlist_and_only_adds_new_songs() {
    let dst = MockApi::new(DST);
    dst.seed_playlist("Mix", vec![dst.catalog_song(1)]);

    sync_into(
        &dst,
        vec![playlist("Mix", source_songs(&[0, 1, 2]))],
        &config(),
    );

    assert_eq!(dst.playlist_count(), 1);
    let synced = dst.playlist("Mix").unwrap();
    assert_eq!(ids(&synced.songs), catalog_ids(&dst, &[1, 0, 2]));
}

#[test]
fn resyncing_is_idempotent() {
    let dst = MockApi::new(DST);
    for _ in 0..3 {
        sync_into(
            &dst,
            vec![playlist("Mix", source_songs(&[0, 1]))],
            &config(),
        );
    }
    assert_eq!(dst.playlist_count(), 1);
    assert_eq!(dst.playlist("Mix").unwrap().songs.len(), 2);
}

#[test]
fn never_removes_songs_from_the_destination() {
    let dst = MockApi::new(DST);
    dst.seed_playlist("Mix", vec![dst.catalog_song(3)]);
    sync_into(&dst, vec![playlist("Mix", source_songs(&[0]))], &config());
    assert_eq!(
        ids(&dst.playlist("Mix").unwrap().songs),
        catalog_ids(&dst, &[3, 0])
    );
}

#[test]
fn skips_platform_generated_and_empty_playlists() {
    let dst = MockApi::new(DST);
    let mut playlists: Vec<Playlist> = SKIPPED_PLAYLISTS
        .iter()
        .map(|name| playlist(name, source_songs(&[0])))
        .collect();
    playlists.push(playlist("Empty", vec![]));
    sync_into(&dst, playlists, &config());
    assert_eq!(dst.playlist_count(), 0);
}

#[test]
fn skips_songs_without_album_or_match() {
    let dst = MockApi::new(DST);
    let mut songs = source_songs(&[0]);
    songs.push(albumless_song(SRC));
    songs.push(unknown_song(SRC, true));
    songs.push(unknown_song(SRC, false));
    sync_into(&dst, vec![playlist("Mix", songs)], &config());
    assert_eq!(
        ids(&dst.playlist("Mix").unwrap().songs),
        catalog_ids(&dst, &[0])
    );
}

#[test]
fn syncs_songs_without_album_when_they_have_an_isrc() {
    let dst = MockApi::new(DST);
    let mut song = FIXTURES[0].song(SRC);
    song.album = None;
    sync_into(&dst, vec![playlist("Mix", vec![song])], &config());
    assert_eq!(
        ids(&dst.playlist("Mix").unwrap().songs),
        catalog_ids(&dst, &[0])
    );
}

#[test]
fn creates_the_playlist_even_when_nothing_matches() {
    let dst = MockApi::new(DST);
    sync_into(
        &dst,
        vec![playlist("Mix", vec![unknown_song(SRC, true)])],
        &config(),
    );
    assert!(dst.playlist("Mix").unwrap().songs.is_empty());
}

#[test]
fn drops_duplicates_within_the_source_playlist() {
    let dst = MockApi::new(DST);
    sync_into(
        &dst,
        vec![playlist("Mix", source_songs(&[0, 1, 0, 1]))],
        &config(),
    );
    assert_eq!(
        ids(&dst.playlist("Mix").unwrap().songs),
        catalog_ids(&dst, &[0, 1])
    );
}

#[test]
fn adds_a_song_once_when_several_source_songs_resolve_to_it() {
    // e.g. the album and the single version of a song
    let dst = MockApi::new(DST);
    let album = FIXTURES[0].song(SRC);
    let mut single = album.clone();
    single.id = "single-version".to_string();
    sync_into(&dst, vec![playlist("Mix", vec![album, single])], &config());
    assert_eq!(dst.playlist("Mix").unwrap().songs.len(), 1);
}

#[test]
fn syncs_platforms_without_isrc_by_metadata() {
    // YtMusic playlists carry no ISRC
    let src = MockApi::new(MusicApiType::YtMusic);
    let dst = MockApi::new(DST);
    let songs = vec![src.catalog_song(0), src.catalog_song(4)];
    assert!(songs.iter().all(|s| s.isrc.is_none()));
    sync_into(&dst, vec![playlist("Mix", songs)], &config());
    assert_eq!(
        ids(&dst.playlist("Mix").unwrap().songs),
        catalog_ids(&dst, &[0, 4])
    );
}

#[test]
fn like_all_likes_added_songs_that_are_not_liked_yet() {
    let dst = MockApi::new(DST);
    dst.seed_likes(vec![dst.catalog_song(1)]);
    let config = ConfigArgs {
        like_all: true,
        ..config()
    };
    sync_into(&dst, vec![playlist("Mix", source_songs(&[0, 1]))], &config);
    assert_eq!(ids(&dst.likes()), catalog_ids(&dst, &[1, 0]));
}

#[test]
fn aborts_when_a_destination_playlist_is_unreadable() {
    // treating it as empty would re-add, and so duplicate, everything in it
    let dst = MockApi::new(DST);
    let id = dst.seed_playlist("Broken", vec![dst.catalog_song(0)]);
    dst.make_unreadable(&id);

    let res = block_on(synchronize_playlists(
        vec![playlist("Mix", source_songs(&[0]))],
        &dst.boxed(),
        &config(),
    ));
    let err = format!("{:?}", res.unwrap_err());
    assert!(
        err.contains("Broken"),
        "error doesn't name the playlist: {err}"
    );
    assert!(dst.playlist("Mix").is_none());
}

#[test]
fn synchronize_skips_unreadable_source_playlists() {
    let src = MockApi::new(SRC);
    let dst = MockApi::new(DST);
    let broken = src.seed_playlist("Broken", vec![src.catalog_song(0)]);
    src.make_unreadable(&broken);
    src.seed_playlist("Fine", vec![src.catalog_song(1)]);

    block_on(synchronize(src.boxed(), dst.boxed(), config())).unwrap();

    assert!(dst.playlist("Broken").is_none());
    assert_eq!(
        ids(&dst.playlist("Fine").unwrap().songs),
        catalog_ids(&dst, &[1])
    );
}

#[test]
fn synchronize_refuses_platforms_in_different_countries() {
    let src = MockApi::new(SRC).with_country(Some("FR"));
    let dst = MockApi::new(DST).with_country(Some("US"));
    src.seed_playlist("Mix", vec![src.catalog_song(0)]);

    let err = block_on(synchronize(src.boxed(), dst.boxed(), config())).unwrap_err();
    assert!(format!("{err}").contains("different countries"));
    assert_eq!(dst.playlist_count(), 0);

    let config = ConfigArgs {
        diff_country: true,
        ..config()
    };
    block_on(synchronize(src.boxed(), dst.boxed(), config)).unwrap();
    assert_eq!(dst.playlist_count(), 1);
}

#[test]
fn synchronize_ignores_an_unknown_country() {
    // e.g. YtMusic, or a Spotify Development Mode app
    for (src_country, dst_country) in [(None, Some("US")), (Some("FR"), None)] {
        let src = MockApi::new(SRC).with_country(src_country);
        let dst = MockApi::new(DST).with_country(dst_country);
        src.seed_playlist("Mix", vec![src.catalog_song(0)]);
        block_on(synchronize(src.boxed(), dst.boxed(), config())).unwrap();
        assert_eq!(dst.playlist("Mix").unwrap().songs.len(), 1);
    }
}

#[test]
fn synchronize_syncs_likes_only_when_asked() {
    let src = MockApi::new(SRC);
    let dst = MockApi::new(DST);
    src.seed_likes(vec![src.catalog_song(0)]);

    block_on(synchronize(src.boxed(), dst.boxed(), config())).unwrap();
    assert!(dst.likes().is_empty());

    let config = ConfigArgs {
        sync_likes: true,
        ..config()
    };
    block_on(synchronize(src.boxed(), dst.boxed(), config)).unwrap();
    assert_eq!(ids(&dst.likes()), catalog_ids(&dst, &[0]));
}

#[test]
fn synchronize_likes_adds_only_missing_likes() {
    let src = MockApi::new(SRC);
    let dst = MockApi::new(DST);
    src.seed_likes(vec![
        src.catalog_song(0),
        src.catalog_song(1),
        unknown_song(SRC, true),
    ]);
    dst.seed_likes(vec![dst.catalog_song(1), dst.catalog_song(3)]);

    block_on(synchronize_likes(&src.boxed(), &dst.boxed())).unwrap();
    // nothing unliked, the missing one added, the unknown one ignored
    assert_eq!(ids(&dst.likes()), catalog_ids(&dst, &[1, 3, 0]));
}

#[test]
fn get_playlists_full_skips_or_fails_on_unreadable_playlists() {
    let api = MockApi::new(DST);
    let broken = api.seed_playlist("Broken", vec![api.catalog_song(0)]);
    api.make_unreadable(&broken);
    api.seed_playlist("Fine", vec![api.catalog_song(1)]);

    let all = block_on(api.get_playlists_full(true)).unwrap();
    let names: Vec<(&str, usize)> = all
        .iter()
        .map(|p| (p.name.as_str(), p.songs.len()))
        .collect();
    assert_eq!(names, [("Broken", 0), ("Fine", 1)]);

    let err = block_on(api.get_playlists_full(false)).unwrap_err();
    assert!(format!("{err:?}").contains("Broken"));
}
