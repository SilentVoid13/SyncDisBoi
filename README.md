# SyncDisBoi

SyncDisBoi is a simple, efficient playlist synchronizer between different music streaming services. It currently supports:
- [Youtube Music](https://music.youtube.com/)
- [Spotify](https://open.spotify.com/)

This project is made for music lovers who want to maintain their playlists across different music streaming services.

> **Disclaimer**: I am not responsible for messing up with your playlists. 
> This tool doesn't perform any deletion operations by default, but always be careful and make backups.

## Accuracy

SyncDisBoi focuses on synchronization accuracy, to ensure that each song on the source playlist matches accurately the corresponding song on the destination playlist. This feature is particularly useful for users who prioritize maintaining the integrity of their playlists and prevent ending up with weird remixes during synchronization.

SyncDisBoi verifies the following properties to ensure that the two songs match:
- Song name resemblance score ([Levenshtein distance](https://en.wikipedia.org/wiki/Levenshtein_distance))
- Album name resemblance score ([Levenshtein distance](https://en.wikipedia.org/wiki/Levenshtein_distance))

For Youtube Music, SyncDisBoi won't sync songs with no album metadata. This indicates a video coming from Youtube, which has no metadata that can be leveraged to accurately sync it.

## Usage

Each Music platform has a different way of setting up account access over their API.

### Youtube Music API

- Over the first run, SyncDisBoi will open up a browser tab to request OAuth access for your Youtube Account.
- Authorize the application in your browser, then press ENTER on the CLI to continue.

The OAuth token will be cached in `~/.config/SyncDisBoi/ytmusic_oauth.json` to avoid having to request access every time SyncDisBoi is run.

Note: By default, SyncDisBoi uses the "Youtube for TV" application to request OAuth access. This is to simplify the process, but you can also create your own OAuth application, grant access to your Youtube account email and then use it in SyncDisBoi by providing its client id and client secret.

### Spotify credentials

- Navigate to [https://developer.spotify.com/](https://developer.spotify.com/)
  and create an application
- Add [https://localhost:8888/callback](https://localhost:8888/callback) as a
  redirect URI in your application settings
- Copy the application client id and client secret

You will then need to provide the client id and client secret as arguments for SyncDisBoi.

## Contributing

SyncDisBoi is designed with flexibility in mind. The architecture enables easy addition of new music platforms: just create a structure that implements the [MusicApi](https://github.com/SilentVoid13/SyncDisBoi/blob/master/src/music_api.rs#L15) trait, add it as an option to the program arguments and you're done!
This makes it a great project for developers to contribute to.
