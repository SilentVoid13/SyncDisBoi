//! The behaviour every [`MusicApi`] implementation must have, written once
//! against the trait and instantiated per platform by [`contract_tests!`] and
//! [`cross_tests!`].
//!
//! Tests use `ensure!` rather than `assert!` so that a failure still runs the
//! cleanup of whatever the test created on the account.

use std::collections::HashSet;

use color_eyre::eyre::{Result, WrapErr, bail, ensure, eyre};
use sync_dis_boi::ConfigArgs;
use sync_dis_boi::music_api::{DynMusicApi, Playlist, Song};
use sync_dis_boi::sync::synchronize_playlists;

use super::fixtures::{FIXTURES, albumless_song, exposes_isrc, probe_source, probes, unknown_song};
use super::util::{
    contains_match, delete_by_name, describe, eventually, find_playlist, first_error, unique_name,
    wait_for_playlist_len, with_playlist,
};

/// Expands to one `#[test]` per contract test, each calling `$run(test_fn)`.
/// Attributes after the runner (`run; #[ignore]`) are put on every test.
#[macro_export]
macro_rules! contract_tests {
    (@each $attrs:tt $run:tt; $($test:ident),* $(,)?) => {
        $( $crate::contract_tests!(@one $attrs $run; $test); )*
    };
    (@one [$(#[$attr:meta])*] ($run:expr); $test:ident) => {
        #[test]
        $(#[$attr])*
        fn $test() {
            ($run)($crate::common::contract::$test)
        }
    };
    ($run:expr) => {
        $crate::contract_tests!($run;);
    };
    ($run:expr; $(#[$attr:meta])*) => {
        $crate::contract_tests!(@each [$(#[$attr])*] ($run);
            authenticates,
            playlist_create_list_delete,
            playlist_add_read_remove,
            playlist_edits_with_no_songs_are_noops,
            playlists_full_includes_songs,
            search_by_isrc,
            search_by_metadata,
            search_unknown_song_finds_nothing,
            search_songs_keeps_order,
            likes_add_read_remove,
            sync_creates_then_updates_playlist,
        );
    };
}

/// Expands to one `#[test]` per listed `name: Src => Dst` platform pair.
#[macro_export]
macro_rules! cross_tests {
    (@each $attrs:tt $run:path; $($name:ident: $src:ident => $dst:ident),* $(,)?) => {
        $( $crate::cross_tests!(@one $attrs $run; $name: $src => $dst); )*
    };
    (@one [$(#[$attr:meta])*] $run:path; $name:ident: $src:ident => $dst:ident) => {
        #[test]
        $(#[$attr])*
        fn $name() {
            $run(
                &sync_dis_boi::music_api::MusicApiType::$src,
                &sync_dis_boi::music_api::MusicApiType::$dst,
                $crate::common::contract::sync_between,
            )
        }
    };
    ($(#[$attr:meta])* $run:path; $($pairs:tt)*) => {
        $crate::cross_tests!(@each [$(#[$attr])*] $run; $($pairs)*);
    };
}

fn sync_config() -> ConfigArgs {
    ConfigArgs {
        debug: false,
        like_all: false,
        sync_likes: false,
        diff_country: true,
        proxy: None,
    }
}

/// Search the first `n` fixtures on `api`; every one of them must be found.
async fn resolve_fixtures(api: &DynMusicApi, n: usize) -> Result<Vec<Song>> {
    let probes = probes(&api.api_type(), n);
    let found = api.search_songs(&probes).await?;
    probes
        .iter()
        .zip(found)
        .map(|(probe, found)| found.ok_or(eyre!("fixture not found on the platform: {probe}")))
        .collect()
}

fn check_song_shape(api: &DynMusicApi, song: &Song) -> Result<()> {
    let kind = api.api_type();
    ensure!(
        song.source == kind,
        "song source is {:?}: {song}",
        song.source
    );
    ensure!(!song.id.is_empty(), "song has no id: {song}");
    ensure!(!song.name.is_empty(), "song has no name: {song:?}");
    ensure!(!song.artists.is_empty(), "song has no artists: {song}");
    ensure!(
        song.album.is_some() || song.isrc.is_some(),
        "song has neither album nor ISRC: {song}"
    );
    if kind.has_duration() {
        ensure!(song.duration_ms > 0, "song has no duration: {song}");
    }
    Ok(())
}

pub async fn authenticates(api: &'static DynMusicApi) -> Result<()> {
    let kind = api.api_type();
    if let Some(country) = api.country_code() {
        ensure!(
            country.len() == 2 && country.chars().all(|c| c.is_ascii_uppercase()),
            "{kind:?} country code is not ISO 3166-1 alpha-2: {country:?}"
        );
    }
    api.get_playlists_info()
        .await
        .wrap_err("listing playlists")?;
    Ok(())
}

pub async fn playlist_create_list_delete(api: &'static DynMusicApi) -> Result<()> {
    let name = unique_name("lifecycle");
    let created = api.create_playlist(&name, false).await?;
    let id = created.id.clone();

    let body = async {
        ensure!(
            created.name == name,
            "created playlist is named {:?}",
            created.name
        );
        ensure!(!created.id.is_empty(), "created playlist has no id");
        ensure!(created.songs.is_empty(), "new playlist has songs");

        let listed = eventually("the new playlist to be listed", || async {
            find_playlist(api, &name).await
        })
        .await?;
        ensure!(
            listed.id == id,
            "listed id {} != created id {id}",
            listed.id
        );
        ensure!(listed.songs.is_empty(), "get_playlists_info returned songs");

        let songs = api.get_playlist_songs(&id).await?;
        ensure!(
            songs.is_empty(),
            "new playlist has songs:\n{}",
            describe(&songs)
        );
        Ok(())
    }
    .await;

    let deleted = api.delete_playlist(created).await;
    first_error(body, deleted)?;

    eventually("the deleted playlist to disappear", || async {
        Ok(find_playlist(api, &name).await?.is_none().then_some(()))
    })
    .await
}

pub async fn playlist_add_read_remove(api: &'static DynMusicApi) -> Result<()> {
    let songs = resolve_fixtures(api, 4).await?;
    with_playlist(api, "add-remove", |mut playlist| async move {
        api.add_songs_to_playlist(&mut playlist, &songs).await?;
        ensure!(
            playlist.songs.len() == songs.len(),
            "add_songs_to_playlist left {} songs in the local playlist, expected {}",
            playlist.songs.len(),
            songs.len()
        );

        let read = wait_for_playlist_len(api, &playlist.id, songs.len()).await?;
        for (i, (added, got)) in songs.iter().zip(&read).enumerate() {
            check_song_shape(api, got)?;
            ensure!(
                added.id == got.id,
                "position {i}: added {added} [{}], read back {got} [{}]\nread back:\n{}",
                added.id,
                got.id,
                describe(&read)
            );
            // required where the platform always has one, and never wrong
            if exposes_isrc(&api.api_type()) || got.isrc.is_some() {
                ensure!(
                    got.isrc.as_deref() == Some(FIXTURES[i].isrc),
                    "position {i}: {got} has ISRC {:?}, expected {}",
                    got.isrc,
                    FIXTURES[i].isrc
                );
            }
        }

        // Remove from the middle and the end: positional APIs get this wrong.
        let removed = [read[1].clone(), read[3].clone()];
        api.remove_songs_from_playlist(&mut playlist, &removed)
            .await?;
        ensure!(
            playlist.songs.len() == 2,
            "remove_songs_from_playlist left {} songs in the local playlist, expected 2",
            playlist.songs.len()
        );
        let left = wait_for_playlist_len(api, &playlist.id, 2).await?;
        let ids: Vec<&str> = left.iter().map(|s| s.id.as_str()).collect();
        ensure!(
            ids == [read[0].id.as_str(), read[2].id.as_str()],
            "wrong songs left after removal:\n{}",
            describe(&left)
        );
        Ok(())
    })
    .await
}

pub async fn playlist_edits_with_no_songs_are_noops(api: &'static DynMusicApi) -> Result<()> {
    let songs = resolve_fixtures(api, 1).await?;
    with_playlist(api, "noop", |mut playlist| async move {
        api.add_songs_to_playlist(&mut playlist, &[])
            .await
            .wrap_err("adding no songs to an empty playlist")?;
        api.add_songs_to_playlist(&mut playlist, &songs).await?;
        let read = wait_for_playlist_len(api, &playlist.id, 1).await?;

        api.add_songs_to_playlist(&mut playlist, &[])
            .await
            .wrap_err("adding no songs")?;
        api.remove_songs_from_playlist(&mut playlist, &[])
            .await
            .wrap_err("removing no songs")?;
        ensure!(playlist.songs.len() == 1, "local playlist changed");

        let after = api.get_playlist_songs(&playlist.id).await?;
        ensure!(
            after.len() == 1 && after[0].id == read[0].id,
            "playlist changed:\n{}",
            describe(&after)
        );
        Ok(())
    })
    .await
}

pub async fn playlists_full_includes_songs(api: &'static DynMusicApi) -> Result<()> {
    let songs = resolve_fixtures(api, 2).await?;
    with_playlist(api, "full", |mut playlist| async move {
        api.add_songs_to_playlist(&mut playlist, &songs).await?;
        wait_for_playlist_len(api, &playlist.id, 2).await?;

        // `false`: sync reads destinations this way, and must not fail on
        // an ordinary account
        let all = api.get_playlists_full(false).await?;
        let ours = all
            .iter()
            .find(|p| p.id == playlist.id)
            .ok_or(eyre!("get_playlists_full is missing {}", playlist.name))?;
        ensure!(ours.name == playlist.name, "renamed to {:?}", ours.name);
        let ids: Vec<&str> = ours.songs.iter().map(|s| s.id.as_str()).collect();
        let want: Vec<&str> = songs.iter().map(|s| s.id.as_str()).collect();
        ensure!(ids == want, "songs {ids:?}, expected {want:?}");
        Ok(())
    })
    .await
}

pub async fn search_by_isrc(api: &'static DynMusicApi) -> Result<()> {
    for fixture in &FIXTURES {
        let probe = fixture.song(probe_source(&api.api_type()));
        let found = api
            .search_song(&probe)
            .await?
            .ok_or(eyre!("ISRC search found nothing for {probe}"))?;
        check_song_shape(api, &found)?;
        if let Some(isrc) = &found.isrc {
            ensure!(
                isrc == fixture.isrc,
                "ISRC search for {} returned {found} with ISRC {isrc}",
                fixture.isrc
            );
        }
        ensure!(
            found.compare(&probe),
            "{found} [{}] doesn't match {probe}",
            found.id
        );
    }
    Ok(())
}

pub async fn search_by_metadata(api: &'static DynMusicApi) -> Result<()> {
    for fixture in &FIXTURES {
        let probe = fixture.song_without_isrc(probe_source(&api.api_type()));
        let found = api
            .search_song(&probe)
            .await?
            .ok_or(eyre!("metadata search found nothing for {probe}"))?;
        check_song_shape(api, &found)?;
        ensure!(
            found.compare(&probe),
            "{found} [{}] doesn't match {probe}",
            found.id
        );
        ensure!(
            found.clean_name() == probe.clean_name(),
            "metadata search for {probe} returned {found}"
        );
    }
    Ok(())
}

pub async fn search_unknown_song_finds_nothing(api: &'static DynMusicApi) -> Result<()> {
    let src = probe_source(&api.api_type());
    for with_isrc in [true, false] {
        let probe = unknown_song(src.clone(), with_isrc);
        if let Some(found) = api.search_song(&probe).await? {
            bail!("search for a song that doesn't exist (isrc: {with_isrc}) returned {found}");
        }
    }
    Ok(())
}

pub async fn search_songs_keeps_order(api: &'static DynMusicApi) -> Result<()> {
    let src = probe_source(&api.api_type());
    let probes = vec![
        FIXTURES[2].song(src.clone()),
        unknown_song(src.clone(), false),
        FIXTURES[0].song_without_isrc(src.clone()),
        FIXTURES[1].song(src),
    ];
    let found = api.search_songs(&probes).await?;
    ensure!(found.len() == probes.len(), "got {} results", found.len());
    for (probe, found) in probes.iter().zip(&found) {
        let expect_found = probe.id != "fixture-unknown";
        match found {
            Some(song) if expect_found => {
                ensure!(
                    song.compare(probe),
                    "result {song} is for another probe than {probe}"
                );
            }
            None if !expect_found => {}
            _ => bail!("probe {probe}: unexpected result {found:?}"),
        }
    }
    Ok(())
}

pub async fn likes_add_read_remove(api: &'static DynMusicApi) -> Result<()> {
    let candidates = resolve_fixtures(api, FIXTURES.len()).await?;
    let likes = api.get_likes().await?;
    for like in &likes {
        ensure!(
            like.source == api.api_type(),
            "liked song from {:?}",
            like.source
        );
    }
    // Only touch a song the account doesn't already like, so that
    // unliking it afterwards leaves the account as it was.
    let Some(song) = candidates
        .into_iter()
        .find(|c| !likes.iter().any(|l| l.id == c.id))
    else {
        bail!("every fixture is already liked on this account; unlike one to run this test");
    };

    api.add_likes(std::slice::from_ref(&song)).await?;
    let body = eventually(&format!("{song} to be liked"), || async {
        let likes = api.get_likes().await?;
        Ok(likes.iter().any(|l| l.id == song.id).then_some(likes))
    })
    .await;
    let body = body.and_then(|after| {
        let entry = after.iter().find(|l| l.id == song.id).unwrap();
        check_song_shape(api, entry)
    });
    let unliked = api.remove_likes(std::slice::from_ref(&song)).await;
    first_error(body, unliked)?;

    eventually(&format!("{song} to be unliked"), || async {
        let likes = api.get_likes().await?;
        Ok((!likes.iter().any(|l| l.id == song.id)).then_some(()))
    })
    .await
}

pub async fn sync_creates_then_updates_playlist(api: &'static DynMusicApi) -> Result<()> {
    let kind = api.api_type();
    let src = probe_source(&kind);
    let name = unique_name("sync");
    let extras = || vec![albumless_song(src.clone()), unknown_song(src.clone(), true)];

    let body = async {
        // 1. first sync creates the playlist with what can be found
        let mut songs = probes(&kind, 3);
        songs.extend(extras());
        let source = Playlist {
            id: String::new(),
            name: name.clone(),
            songs: songs.clone(),
        };
        synchronize_playlists(vec![source], api, &sync_config()).await?;

        let dst = eventually("the synced playlist to be listed", || async {
            find_playlist(api, &name).await
        })
        .await?;
        let read = wait_for_playlist_len(api, &dst.id, 3).await?;
        for probe in &songs[..3] {
            ensure!(
                contains_match(&read, probe),
                "{probe} is missing from the synced playlist:\n{}",
                describe(&read)
            );
        }

        // 2. syncing again with one more song only adds that song
        let mut songs = probes(&kind, 4);
        songs.extend(extras());
        let source = Playlist {
            id: String::new(),
            name: name.clone(),
            songs: songs.clone(),
        };
        synchronize_playlists(vec![source], api, &sync_config()).await?;

        let read = wait_for_playlist_len(api, &dst.id, 4).await?;
        ensure!(
            contains_match(&read, &songs[3]),
            "{} is missing after the second sync:\n{}",
            songs[3],
            describe(&read)
        );
        let ids: HashSet<&str> = read.iter().map(|s| s.id.as_str()).collect();
        ensure!(
            ids.len() == 4,
            "duplicates after resync:\n{}",
            describe(&read)
        );

        let listed = api.get_playlists_info().await?;
        let copies = listed.iter().filter(|p| p.name == name).count();
        ensure!(
            copies == 1,
            "resync created {copies} playlists named {name}"
        );
        Ok(())
    }
    .await;

    first_error(body, delete_by_name(api, &name).await)
}

/// Sync a playlist read from `src` into `dst`: every song must land in the
/// destination as whatever `dst` resolves it to, exactly once, and
/// re-syncing must change nothing.
pub async fn sync_between(src: &'static DynMusicApi, dst: &'static DynMusicApi) -> Result<()> {
    const N: usize = 3;
    let songs = resolve_fixtures(src, N).await?;
    // Sync reads songs out of playlists, and some platforms return less there
    // than from search (YtMusic has no ISRC), so the source songs go through
    // a real playlist too.
    let src_songs = with_playlist(src, "cross-src", |mut playlist| async move {
        src.add_songs_to_playlist(&mut playlist, &songs).await?;
        wait_for_playlist_len(src, &playlist.id, N).await
    })
    .await?;

    let name = unique_name("cross-dst");
    let body = async {
        let expected: Vec<Song> = dst
            .search_songs(&src_songs)
            .await?
            .into_iter()
            .zip(&src_songs)
            .map(|(found, song)| found.ok_or(eyre!("{song} can't be found on the destination")))
            .collect::<Result<_>>()?;
        for (song, found) in src_songs.iter().zip(&expected) {
            if let (Some(a), Some(b)) = (&song.isrc, &found.isrc) {
                ensure!(a == b, "{song} ({a}) resolved to {found} ({b})");
            }
        }
        for round in 1..=2 {
            let source = Playlist {
                id: String::new(),
                name: name.clone(),
                songs: src_songs.clone(),
            };
            synchronize_playlists(vec![source], dst, &sync_config()).await?;

            let playlist = eventually("the synced playlist to be listed", || async {
                find_playlist(dst, &name).await
            })
            .await?;
            let read = wait_for_playlist_len(dst, &playlist.id, N)
                .await
                .wrap_err(format!("sync round {round}"))?;
            // A platform can hold several valid copies of a song (YtMusic
            // has the album track and the "song" upload), and which one a
            // search returns varies, so any copy that matches will do.
            for (song, found) in src_songs.iter().zip(&expected) {
                ensure!(
                    read.iter().any(|r| r.id == found.id || song.compare(r)),
                    "sync round {round}: {song} is missing; destination holds\n{}\nexpected\n{}",
                    describe(&read),
                    describe(&expected)
                );
            }
            let ids: HashSet<&str> = read.iter().map(|s| s.id.as_str()).collect();
            ensure!(
                ids.len() == N,
                "sync round {round}: duplicates in\n{}",
                describe(&read)
            );
        }
        Ok(())
    }
    .await;

    first_error(body, delete_by_name(dst, &name).await)
}
