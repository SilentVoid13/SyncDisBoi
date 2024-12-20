set dotenv-load

d_sp2yt:
    cargo run --release -- --debug -l debug --sync-likes --like-all spotify yt-music
d_sp2ti:
    cargo run --release -- --debug -l debug --sync-likes --like-all spotify tidal
d_yt2sp:
    cargo run --release -- --debug -l debug --sync-likes --like-all yt-music spotify
d_yt2ti:
    cargo run --release -- --debug -l debug --sync-likes --like-all yt-music tidal
d_ti2yt:
    cargo run --release -- --debug -l debug --sync-likes --like-all tidal yt-music
d_ti2sp:
    cargo run --release -- --debug -l debug --sync-likes --like-all tidal spotify
