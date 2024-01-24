# SyncDisBoi

SyncDisBoi is a simple and efficient tool playlist designed to synchronize playlists across different music streaming platforms. It currently supports:
- [Youtube Music](https://music.youtube.com/)
- [Spotify](https://open.spotify.com/)

It's the perfect solution for music enthusiasts who want to keep their playlists updated across various music streaming platforms and enjoy the various recommendations algorithms.

> **Disclaimer**: While SyncDisBoi doesn't perform any deletion operations, it's always a good practice to backup your playlists. I am not responsible for any unintended changes to your playlists.

## Accuracy

SyncDisBoi focuses on synchronization accuracy, ensuring that each song on the source playlist accurately matches the corresponding song on the destination playlist. This feature is particularly useful for users who prioritize maintaining the integrity of their playlists and avoid ending up with unexpected remixes during synchronization.

SyncDisBoi verifies the following properties to ensure that the two songs match:
- Song name resemblance score ([Levenshtein distance](https://en.wikipedia.org/wiki/Levenshtein_distance))
- Album name resemblance score ([Levenshtein distance](https://en.wikipedia.org/wiki/Levenshtein_distance))

Notes:
- The duration is not used because some songs have incorrect durations with the Youtube Music API.
- The artist names are not used either because the metadata is inconsistent across platforms. 
- For Youtube Music, SyncDisBoi won't sync songs lacking album metadata, as this typically indicates a video from Youtube, which lacks the necessary metadata for accurate synchronization.

## Usage

Example command to convert your playlists from Youtube Music to Spotify:
```bash
cargo run --release -- yt-music spotify --client-id "<CLIENT_ID>" --client-secret "<CLIENT_SECRET>"
```

To use SyncDisBoi, you need to set up account access for the API of the corresponding music platform.

### Youtube Music API

- On the first run, SyncDisBoi will open up a browser tab to request OAuth access for your Youtube Account.
- Authorize the application in your browser, then press ENTER in the CLI to continue.

The OAuth token will be cached in `~/.config/SyncDisBoi/ytmusic_oauth.json` (on Linux) for future use.

Notes:
- By default, SyncDisBoi uses the "Youtube for TV" application credentials to request OAuth access.
- However, you can also create your own OAuth application, grant access to your account email, and then use it in SyncDisBoi by providing its client id and client secret.

### Spotify

- Visit [https://developer.spotify.com/](https://developer.spotify.com/)
  and create an application.
- Add [https://localhost:8888/callback](https://localhost:8888/callback) as a
  redirect URI in your application settings.
- Copy the application client id and client secret.

You will then need to provide the client id and client secret as arguments for SyncDisBoi.

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
