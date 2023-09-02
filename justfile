set dotenv-load

sp2yt:
    cargo run --release -- -s yt-music -d spotify
yt2sp:
    cargo run --release -- -s spotify -d yt-music
d_sp2yt:
    cargo run --release -- -s yt-music -d spotify --debug
d_yt2sp:
    cargo run --release -- -s spotify -d yt-music --debug
test:
    cargo test --release -- --nocapture
