set dotenv-load

d_sp2yt: d_clean
    cargo run -- --debug -l debug --sync-likes --like-all spotify yt-music
d_sp2ti: d_clean
    cargo run -- --debug -l debug --sync-likes --like-all spotify tidal
d_sp2export: d_clean
    cargo run -- --debug -l debug spotify export -d spotify.json
d_yt2sp: d_clean
    cargo run -- --debug -l debug --sync-likes --like-all yt-music spotify
d_yt2ti: d_clean
    cargo run -- --debug -l debug --sync-likes --like-all yt-music tidal
d_yt2export: d_clean
    cargo run -- --debug -l debug yt-music export -d yt_music.json
d_ti2yt: d_clean
    cargo run -- --debug -l debug --sync-likes --like-all tidal yt-music
d_ti2sp: d_clean
    cargo run -- --debug -l debug --sync-likes --like-all tidal spotify
d_ti2export: d_clean
    cargo run -- --debug -l debug tidal export -d tidal.json
d_clean:
    rm -rf debug
