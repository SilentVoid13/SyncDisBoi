set dotenv-load

sp2yt:
    cargo run --release -- -s yt-music -d spotify
yt2sp:
    cargo run --release -- -s spotify -d yt-music
