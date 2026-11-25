# Pubseed

Pubseed is a desktop client for [AudioPub](https://audiopub.site), the audio sharing and live streaming site. You can browse and play uploads, post your own, read and post comments, favorite uploads, subscribe to people, and sit in on live streams with the chat read out to you as it comes in.

It was built for screen reader users first. Everything can be done from the keyboard, every command lives in the menu bar with its shortcut next to it, and chat messages can be spoken through NVDA, JAWS or Narrator, or through any SAPI voice you have installed if you'd rather keep your screen reader free. It also follows the Windows dark mode setting.

Pubseed is in early development and only runs on Windows for now. Broadcasting your own streams isn't part of it yet; if you want to stream to AudioPub, have a look at [Pubsplash](https://github.com/cha0t1cnu3tral/pubsplash).

## Installing

Download the latest `pubseed-setup` program from the [releases page](https://github.com/seediffusion/pubseed/releases) and run it. It installs Pubseed for everyone on the computer and adds it to the Start menu; if you don't have administrator rights, it offers to install it just for you instead. Running a newer setup program later upgrades the copy you have, and your settings and sign-in are kept.

Pubseed looks for a new version each time it starts, and Help, Check for updates looks straight away. If there is one, it tells you what's new and asks whether to install it. The update downloads, installs out of sight and starts Pubseed again; if Pubseed is installed for everyone, Windows asks for permission first. Settings has a box to turn the check at startup off.

## Getting around

Pubseed has its own help page, which you can open with F1 or from the Help menu. It goes through everything in more detail than this readme.

The window has four tabs: Home, Subscriptions, Live and Notifications. Ctrl+1 to Ctrl+4 jump between them, and the Notifications tab shows how many you haven't read yet.

Pages open inside the tab you're on, a bit like a web browser. Press Enter on an upload and its page opens with the description, Play, Favorite and More buttons, the comments, and a box for writing your own. Escape or Alt+Left takes you back to where you were. F6 moves between the tab strip, the page and the player at the bottom of the window.

Every list works the same way. Enter opens whatever is selected (or joins a stream, or replies to a comment), Space plays or pauses, and the Applications key or Shift+F10 brings up everything else you can do with that item. Keep arrowing past the end of a feed and the next page loads.

When you join a live stream, its audio starts playing and the Live tab turns into the stream's room, with the chat and a box to write in. You can switch to other tabs and keep listening. New messages are read aloud unless you turn that off with Ctrl+Shift+R or in Settings.

## Keyboard shortcuts

| Keys | What it does |
| --- | --- |
| Ctrl+1 to Ctrl+4 | Home, Subscriptions, Live, Notifications |
| F5 | Refresh the page |
| Escape or Alt+Left | Go back |
| F6 and Shift+F6 | Next and previous part of the window |
| Ctrl+U | Your profile |
| Ctrl+B | Your favorites |
| Ctrl+F | Search |
| Ctrl+N | New audio |
| Ctrl+, | Settings |
| Ctrl+Q | Exit |
| Ctrl+P | Play or pause |
| Ctrl+. | Stop |
| Alt+Shift+Left and Right | Skip back and forward |
| Alt+Shift+Page Up and Page Down | Previous and next in the queue |
| Alt+Shift+Up and Down | Volume |
| Alt+. and Alt+, | Faster and slower |
| Ctrl+Shift+W | What's playing |
| Ctrl+Shift+R | Read chat aloud on or off |
| Ctrl+Shift+S | How many people are listening |
| Ctrl+Shift+L | Leave the stream |
| F1 | Pubseed help |

In a list, Left and Right skip through whatever is playing and Delete removes your own comment or a notification, and Clear all on a notification's menu removes every one of them. Save audio, on an upload's menu or under More on its page, keeps a copy of the original file wherever you choose, and remembers the folder for next time. When a comment or chat message has a web link in it, its menu has Open link, which shows you the whole address before opening it in your browser. On the Position slider, the arrows move 5 seconds, Page Up and Page Down move 30, Home and End go to the start and end, and the number keys jump through the audio (5 is halfway).

## Posting audio

File, New audio (Ctrl+N) opens a window for a new upload. It's a separate window, so you can carry on using the rest of Pubseed while it's open, and Ctrl+N brings it back if it's behind something.

Under "Audio from", choose whether to record it or use a file on your computer. Recording uses your default microphone: press Record to start, Pause if you need a moment, and Stop recording when you're done, then Play back to hear it or Discard to start over. Anything shorter than two seconds isn't kept. A file can be in any format AudioPub accepts, up to 500 MB.

Give it a title, between 3 and 120 characters, and a description if you like, then press Publish. The Audio field shows how the upload is going, and once it's done the window closes and the new audio's page opens. Closing the window with something in it asks before throwing it away.

To change or remove something you've posted, use Edit or Delete on its menu in a list (the Applications key), or under More on its page. Delete on its row works too, and it always asks first. AudioPub allows three edits to a post's title and description.

Recordings are saved as WAV, which is simple but large, at roughly 5.5 MB a minute for a mono microphone. That puts the 500 MB limit at about an hour and a half. AudioPub converts uploads itself, so the format only affects how long the upload takes.

## Signing in

Use File, Sign in with your AudioPub email and password. Your password is only sent to AudioPub to sign you in and is never saved. What Pubseed keeps is the session AudioPub gives back, and that goes in Windows Credential Manager under "Pubseed: https://audiopub.site" rather than in a file. Signing out deletes it.

Your other settings are in `%APPDATA%\pubseed\settings.json`.

## Building

You'll need:

- Windows 10 or 11
- A recent stable Rust toolchain (the project uses the 2024 edition)
- Visual Studio Build Tools with the C++ workload
- CMake
- LLVM, for the libclang that wxDragon uses to generate its bindings
- Ninja, which isn't strictly required but makes the wxWidgets build a lot faster

Then:

```
cargo build --release
```

The first build downloads the wxWidgets source and compiles it, which takes several minutes. Builds after that are quick. `cargo run` starts the app.

To make the setup program, install [Inno Setup 6](https://jrsoftware.org/isinfo.php) and, after a release build, run:

```
iscc installer\pubseed.iss
```

It's written to `target\installer`. The version comes from `Cargo.toml`.

### Publishing a release

Pubseed's updater looks at the latest release on GitHub, so a release has to be published as a full release, not a pre-release, and tagged with its version, such as `v0.3.0`. It needs two files: `pubseed_setup.exe`, and its signature, `pubseed_setup.exe.minisig`, which [minisign](https://jedisct1.github.io/minisign/) makes with the release key:

```
minisign -Sm target\installer\pubseed_setup.exe -s %USERPROFILE%\.minisign\pubseed.key
```

Pubseed only installs an update whose signature matches the public key in `installer\minisign.pub`, which is built into the program. The release notes are shown in the update dialog.

`cargo test` runs the offline tests. A couple of tests talk to the real AudioPub site and play audio, so they're skipped by default; run them with `cargo test -- --ignored`.

## How it talks to AudioPub

AudioPub doesn't have a public API, so Pubseed uses the same endpoints the website does: the `__data.json` page data that SvelteKit serves, the site's form actions for things like favoriting and commenting, and the live events feed for chat. If the site changes, parts of Pubseed may stop working until they're updated. The site's source is at [the-byte-bender/audiopub-sv](https://github.com/the-byte-bender/audiopub-sv).

## Thanks

The way Pubseed gets wxWidgets controls to read properly with screen readers, and the way it speaks through them, comes from Pubsplash, by Brandon Cross.

## Licence

Pubseed is released under the MIT License; see `LICENSE`. Code and libraries from other projects keep their own licences, which are covered in `THIRD-PARTY-NOTICES.md`.
