set dotenv-load

sp2yt:
    cargo run --release -- spotify yt-music --headers ./browser.json
yt2sp:
    cargo run --release -- yt-music spotify
yt2ti:
    cargo run --release -- yt-music tidal
ti2yt:
    cargo run --release -- tidal yt-music
d_sp2yt:
    cargo run --release -- --debug -l debug spotify yt-music
d_yt2sp:
    cargo run --release -- --debug -l debug yt-music spotify
d_yt2ti:
    cargo run --release -- --debug -l debug yt-music tidal
d_ti2yt:
    cargo run --release -- --debug -l debug tidal yt-music
test:
    cargo test --release -- --nocapture
