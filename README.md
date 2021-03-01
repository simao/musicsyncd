# musicsyncd - sync your music to your phone

Runs a daemon to serve a music directory to your android phone running [music-sync-android](https://github.com/simao/music-sync-android).

Currently existing solutions to sync a music library to your android phone are too complex or do not fit my use case. These are my requirements:

- I want to play my own music files on my phone, I don't want to use some streaming/download service. I already have a local music library managed by [beets](http://beets.io/), so I just need to sync that to my phone.

- I want to be able to use any music application on my phone, I don't want to depend on some bad app for listening/library management just because it syncs with my computer.

- My music library is album oriented, when I want to sync music I usually sync entire albums. I want be able to easily choose which albums I want synced to my phone.

`musicsyncd` finds albums in a directory organized by `Artist/Albums/Tracks` and serves it to music-sync-android. After downloading your files to your android you can use any music application to listen to your music.

## Running

Curently you will need to build this app from source, run `cargo build --release`. The `musicsyncd` binary will be in `target/release`.

Run `musicsyncd --help` to see options. For most cases you can just run `musicsyncd -m <path to your music dir>`. 

Then you can run music-sync-android and configure it to use the ip/port of the machine running `musicsyncd`. Make sure your firewall allows access to the port `musicsyncd` is biding to. You can change the port used by `musicsyncd` with `--port`.
