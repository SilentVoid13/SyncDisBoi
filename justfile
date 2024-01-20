set dotenv-load

sp2yt:
    cargo run --release -- -s spotify -d yt-music
yt2sp:
    cargo run --release -- -s yt-music -d spotify
d_sp2yt:
    cargo run --release -- -s spotify -d yt-music --debug
d_yt2sp:
    cargo run --release -- -s yt-music -d spotify --debug
test:
    cargo test --release -- --nocapture
