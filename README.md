# SyncDisBoi

SyncDisBoi is a simple and efficient tool designed to synchronize playlists across different music streaming platforms. It currently supports:
- [Youtube Music](https://music.youtube.com/)
- [Spotify](https://open.spotify.com/)
- [Tidal](https://tidal.com/)

SyncDisBoi is the ideal tool for music enthusiasts who want to:
- Seamlessly migrate to a new music platform while preserving their playlists and likes
- Keep playlists in sync across multiple platforms and enjoy each platform's unique recommendation algorithms
- Export existing playlists in a portable JSON format for easy backup or sharing

> **Disclaimer**: While SyncDisBoi doesn't perform any deletion operations, it's always a good practice to backup your playlists. I am not responsible for any unintended changes to your playlists.

## Accuracy

SyncDisBoi focuses on synchronization accuracy, ensuring that each track on the source playlist accurately matches the corresponding track on the destination playlist. This feature is particularly useful for users who prioritize maintaining the integrity of their playlists and avoid ending up with unexpected remixes during synchronization.

If available, SyncDisBoi uses the [International Standard Recording Code (ISRC)](https://en.wikipedia.org/wiki/International_Standard_Recording_Code) to guarantee correct track matching.

When ISRC codes are not available on the platform API, SyncDisBoi falls back to verifying the following properties to ensure that the two tracks match:
- Song name resemblance score ([Levenshtein distance](https://en.wikipedia.org/wiki/Levenshtein_distance))
- Album name resemblance score ([Levenshtein distance](https://en.wikipedia.org/wiki/Levenshtein_distance))
- Song duration

Notes:
- The artist names are not used because the metadata is inconsistent across platforms. 
- For Youtube Music, SyncDisBoi won't sync tracks lacking album metadata, as this typically indicates a video from Youtube, which lacks the necessary metadata for accurate synchronization.

## Download and Build

Pre-built binaries of SyncDisBoi for Linux, Windows, and macOS are available under the [releases](https://github.com/SilentVoid13/SyncDisBoi/releases) section.

If you prefer to build SyncDisBoi from source you simply need the rust toolchain, e.g. available via [rustup](https://rustup.rs/).
A [Nix flake](https://github.com/SilentVoid13/SyncDisBoi/blob/master/flake.nix) is also available with a pre-configured environment with support for cross-compilation.

## Usage

Here are some command examples:
```bash
# sync from Youtube Music to Spotify
./sync_dis_boi yt-music --headers "./browser.json" spotify --client-id "<CLIENT_ID>" --client-secret "<CLIENT_SECRET>"
# sync from Spotify to Tidal, sync likes as well
./sync_dis_boi --sync-likes spotify --client-id "<CLIENT_ID>" --client-secret "<CLIENT_SECRET>" tidal
# sync from Tidal to Youtube Music, like all synchronized songs
./sync_dis_boi --like-all tidal yt-music --headers "./browser.json"
# sync from Spotify to Youtube Music, with debug mode enabled to generate detailed statistics about the synchronization process
./sync_dis_boi --debug spotify --client-id "<CLIENT_ID>" --client-secret "<CLIENT_SECRET>" yt-music --headers "./browser.json"

# export Spotify playlists to JSON
./sync_dis_boi spotify --client-id "<CLIENT_ID>" --client-secret "<CLIENT_SECRET>" export -d ./spotify.json
# export Youtube Music playlists to JSON
./sync_dis_boi yt-music --headers "./browser.json" export -d ./yt_music.json
# export Tidal playlists to JSON
./sync_dis_boi tidal export -d ./tidal.json
```

To use SyncDisBoi, you need to set up account access for the API of the corresponding music platform.

### Spotify API setup

- Visit [https://developer.spotify.com/](https://developer.spotify.com/)
  and create an application.
- Add [http://localhost:8888/callback](http://localhost:8888/callback) as a
  redirect URI in your application settings.
- Copy the application client id and client secret.

You will then need to provide the client id and client secret as arguments for SyncDisBoi.

Notes:
- After authorizing access for your Spotify account, SyncDisBoi will open the 'http://localhost:8888/callback' URL in your browser. If you get 'Unable to connect' this is normal as the server is quickly opened and shutdown once it receives the auth code.

### Youtube Music API setup

The OAuth access has been removed by Youtube. You should now log in using your browser cookies:

- Follow [ytmusicapi's guide](https://ytmusicapi.readthedocs.io/en/stable/setup/browser.html) to generate a `browser.json` file.
- Pass the `browser.json` file as an argument for SyncDisBoi

Notes:
- You may have to refresh the cookies regularly as they can expire

### Tidal API setup

- On the first run, SyncDisBoi will open up a browser tab to request OAuth access for your Tidal Account.
- Authorize the application in your browser, then press ENTER in the CLI to continue.

The OAuth token will be cached in `~/.config/SyncDisBoi/tidal_oauth.json` (on Linux) for future use.

Notes:
- By default, SyncDisBoi uses Tidal's "Android Auto" application credentials to request OAuth access.
- However, you can also create your own Tidal application and then use it in SyncDisBoi by providing its client id and client secret.

### Debug mode

You can enable debug mode (`--debug`) to generate detailed statistics about the synchronization process.

Files are saved in the `debug/` folder:
- `conversion_rate.json`: success rate of song synchronization
- `missing_songs.json`: list of tracks that couldn’t be synchronized
- `new_songs.json`: list of tracks successfully synchronized
- `songs_with_no_albums.json`: list of songs skipped due to missing album metadata

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for details on how to contribute to this project.

## License

SyncDisBoi is licensed under the GNU AGPLv3 license. Refer to [LICENSE](LICENSE.txt) for more information.

## Support

Your support helps me continue to maintain and improve this project. If you find SyncDisBoi useful and want to show your appreciation, consider sponsoring or donating:
- GitHub Sponsors: Preferred method. You can sponsor me on [GitHub Sponsors](https://github.com/sponsors/SilentVoid13). 
- PayPal: You can also make a donation via [PayPal](https://www.paypal.com/donate?hosted_button_id=U2SRGAFYXT32Q).

Every bit of support is greatly appreciated!

[![GitHub Sponsors](https://img.shields.io/github/sponsors/silentvoid13?label=Sponsor&logo=GitHub%20Sponsors&style=for-the-badge)](https://github.com/sponsors/silentvoid13)
[![Paypal](https://img.shields.io/badge/paypal-silentvoid13-yellow?style=social&logo=paypal)](https://www.paypal.com/donate?hosted_button_id=U2SRGAFYXT32Q)
