# SyncDisBoi

SyncDisBoi is a simple, efficient playlist synchronizer between different
music streaming services. It currently supports:
- Youtube Music
- Spotify

This project is made for music lovers who want to maintain their playlists across different music streaming services.

## Synchronization precision

SyncDisBoi focuses on precision to ensure that each song on the source playlist matches accurately to the corresponding song on the destination playlist. This feature is particularly useful for users who prioritize maintaining the integrity of their playlists and avoid ending up with weird remixes during synchronization.

SyncDisBoi verifies the following properties to ensure that the two songs match:
- Song name resemblance score ([Levenshtein distance](https://en.wikipedia.org/wiki/Levenshtein_distance))
- Album name resemblance score ([Levenshtein distance](https://en.wikipedia.org/wiki/Levenshtein_distance))

For Youtube Music, SyncDisBoi won't sync songs with no albums defined. That's because this means that the song is a video from Youtube and has no metadata that can be leveraged to accurately sync it.

## Flexibility

SyncDisBoi is designed with flexibility in mind. The tool's architecture enables easy addition of new music platforms, making it a great project for developers to contribute to.

## Usage

For more information on the way to get your credentials for the different
music platforms API, see below.

```txt
Usage: sync_dis_boi --yt-music-secret <YT_MUSIC_SECRET> --yt-music-cookies <YT_MUSIC_COOKIES> --spotify-client-id <SPOTIFY_CLIENT_ID> --spotify-client-secret <SPOTIFY_CLIENT_SECRET>

Options:
      --yt-music-secret <YT_MUSIC_SECRET>
          The secret for the YouTube Music API [env: YT_MUSIC_SECRET=]
      --yt-music-cookies <YT_MUSIC_COOKIES>
          The path to the cookies file for the YouTube Music API [env: YT_MUSIC_COOKIES=]
      --spotify-client-id <SPOTIFY_CLIENT_ID>
          The client ID for the Spotify API application [env: SPOTIFY_CLIENT_ID=]
      --spotify-client-secret <SPOTIFY_CLIENT_SECRET>
          The client secret for the Spotify API application [env: SPOTIFY_CLIENT_SECRET=]
  -h, --help
          Print help
  -V, --version
          Print version
```

## Youtube Music credentials

To authorize SyncDisBoi to access your Youtube Music account, it's
currently a bit of a hassle since it has no official API.
- Navigate to [https://music.youtube.com](https://music.youtube.com) and login to your account
- Open up the developer tools network tab and intercept a request while
  browsing the website. Copy the `key` GET parameter from that request.
- Export your cookies into a text file, in the `KEY1=value1;KEY2=value2` format. This can be done using the developer tools or with an extension such as [EditThisCookie](https://chrome.google.com/webstore/detail/editthiscookie/fngmhnnpilhplaeedifhccceomclgfbg?hl=en)

## Spotify credentials

- Navigate to [https://developer.spotify.com/](https://developer.spotify.com/)
  and create an application
- Add [https://localhost:8888/callback](https://localhost:8888/callback) as a
  redirect URI in your application settings
- Copy the application client id and client secret
