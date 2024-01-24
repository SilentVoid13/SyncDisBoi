set dotenv-load

sp2yt:
    cargo run --release -- spotify yt-music
yt2sp:
    cargo run --release -- yt-music spotify
d_sp2yt:
    cargo run --release -- --debug -l debug spotify yt-music
d_yt2sp:
    cargo run --release -- --debug -l debug yt-music spotify
test:
    cargo test --release -- --nocapture
