set dotenv-load

# personal, gitignored recipes
import? 'local.just'

d_sp2yt: d_clean
    cargo run -- --debug -l debug --sync-likes --like-all spotify yt-music
d_sp2ti: d_clean
    cargo run -- --debug -l debug --sync-likes --like-all spotify tidal
d_sp2export: d_clean
    cargo run -- --debug -l debug spotify export -o spotify.json
d_yt2sp: d_clean
    cargo run -- --debug -l debug --sync-likes --like-all yt-music spotify
d_yt2ti: d_clean
    cargo run -- --debug -l debug --sync-likes --like-all yt-music tidal
d_yt2export: d_clean
    cargo run -- --debug -l debug yt-music export -o yt_music.json
d_ti2yt: d_clean
    cargo run -- --debug -l debug --sync-likes --like-all tidal yt-music
d_ti2sp: d_clean
    cargo run -- --debug -l debug --sync-likes --like-all tidal spotify
d_ti2export: d_clean
    cargo run -- --debug -l debug tidal export -o tidal.json
d_sp2import: d_clean
    cargo run -- --debug -l debug spotify import -i spotify.json
d_sp2lb: d_clean
    cargo run -- --debug -l debug spotify listenbrainz
d_ti2lb: d_clean
    cargo run -- --debug -l debug tidal listenbrainz
d_lb2export: d_clean
    cargo run -- --debug -l debug listenbrainz export -o listenbrainz.json
d_lb2import: d_clean
    cargo run -- --debug -l debug listenbrainz import -i canon.json
d_clean:
    rm -rf debug
test:
    cargo test
# live suite on the real accounts; filter e.g. `tidal::`, `cross::`, `spotify::search`
test-live filter="":
    cargo test --test live -- --ignored {{filter}}
export_all: d_sp2export d_yt2export d_ti2export d_lb2export
