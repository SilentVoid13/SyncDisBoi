set dotenv-load

sp2yt:
    cargo run --release -- spotify yt-music
yt2sp:
    cargo run --release -- yt-music spotify
yt2ti:
    cargo run --release -- yt-music tidal
d_sp2yt:
    cargo run --release -- --debug -l debug spotify yt-music
d_yt2sp:
    cargo run --release -- --debug -l debug yt-music spotify
d_yt2ti:
    cargo run --release -- --debug -l debug yt-music tidal
test:
    cargo test --release -- --nocapture
