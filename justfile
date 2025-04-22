set dotenv-load

d_sp2yt:
    cargo run --release -- --debug -l debug --sync-likes --like-all spotify yt-music
d_sp2ti:
    cargo run --release -- --debug -l debug --sync-likes --like-all spotify tidal
d_sp2export:
    cargo run --release -- --debug -l debug spotify export -d spotify.json
d_yt2sp:
    cargo run --release -- --debug -l debug --sync-likes --like-all yt-music spotify
d_yt2ti:
    cargo run --release -- --debug -l debug --sync-likes --like-all yt-music tidal
d_yt2export:
    cargo run --release -- --debug -l debug yt-music export -d yt_music.json
d_ti2yt:
    cargo run --release -- --debug -l debug --sync-likes --like-all tidal yt-music
d_ti2sp:
    cargo run --release -- --debug -l debug --sync-likes --like-all tidal spotify
d_ti2export:
    cargo run --release -- --debug -l debug tidal export -d tidal.json
