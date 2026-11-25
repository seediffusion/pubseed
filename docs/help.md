# Pubseed help

Pubseed is a desktop program for [AudioPub](https://audiopub.site), the site for sharing audio and streaming live. You can listen to what people post, post your own, comment, keep favorites, subscribe to people, and sit in on live streams with the chat read out to you. Everything works from the keyboard, and it was made with screen readers in mind from the start.

You can open this page at any time with F1, or from Help, Pubseed help.

## Getting started

### Installing and updating

Pubseed is installed with its setup program, from the [releases page](https://github.com/seediffusion/pubseed/releases). Pubseed looks for a new version each time it starts, and Help, Check for updates looks straight away. When there is one, it tells you what's new and asks whether to install it. If you say yes, it downloads the update, closes, installs it without any setup windows, and opens again. When Pubseed is installed for everyone on the computer, Windows asks for permission before the update installs. Your settings and sign-in stay as they were.

Every update is checked against Pubseed's signature before it's installed, so a file that didn't come from Pubseed's developers is never run. To stop the check at startup, turn off "Check for updates when Pubseed starts" in Settings.

### Signing in

You can browse and listen without an account. To comment, favorite, subscribe, post or chat, use File, Sign in, and enter the email address and password you use on AudioPub. Your password is sent to AudioPub to sign you in and is never saved. What Pubseed does keep is the session AudioPub gives back, and that goes into Windows Credential Manager, not into a file. File, Sign out removes it.

### Your screen reader

Pubseed works with NVDA, JAWS and Narrator. Things it tells you, like "Paused" or "Volume 80", are spoken through your screen reader and also shown in the status bar at the bottom of the window, so you can review the last one there. Live chat can be read by your screen reader or by a separate Windows voice; see [Settings](#settings).

## Finding your way around

### The main window

The window has four tabs: Home, Subscriptions, Live and Notifications. Ctrl+1 to Ctrl+4 go straight to each one. The Notifications tab shows how many notifications you haven't read. Pubseed only runs once: starting it again while it's open brings its window to the front.

The window is split into regions: the tabs, the page you're on, and the player, which appears at the bottom once something is playing. F6 moves to the next region and Shift+F6 to the previous one.

Pages open inside the tab you're on, much like a web browser. Opening an audio, a person's profile or your favorites puts a new page on top, and Escape or Alt+Left takes you back to the page before. F5 loads the page again.

### Lists

Most of Pubseed is lists, and they all work the same way:

- Enter opens what's selected. On a stream it joins it, and on a comment it starts a reply.
- Space plays or pauses the selected audio.
- Left and Right skip back and forward through whatever is playing.
- Delete removes your own comment, your own audio or a notification. It always asks first when something can't be brought back.
- The Applications key or Shift+F10 shows everything else you can do with the selected row.

Feeds load a page at a time. Arrow down onto the last row and the next page loads; Pubseed tells you how many more arrived, or "No more audios" when you've reached the end.

Home has a Sort by box above the list, for the newest, the oldest, the most played, the most favorited, by title, or in random order. Pubseed remembers your choice.

Every three minutes Pubseed checks the feed you're looking at. Play and favorite counts are brought up to date where they are, and if there's something new it says so, but it leaves the list alone so nothing moves under you. Press F5 when you want to see the new posts.

The row for whatever is playing ends with "playing" or "paused", so you can find it again.

### Profiles, favorites and search

Ctrl+U opens your profile, with everything you've posted, and Ctrl+B opens your favorites. Anyone else's profile is under View profile on the menu of something they posted or wrote. A profile page has a Subscribe button; once you subscribe to someone, what they post shows up on your Subscriptions tab.

Ctrl+F searches AudioPub. Type at least three characters.

## Listening

Press Space on an audio to play it, and again to pause. To listen to a whole list, use Play all from here on the menu of the audio you want to start with: Pubseed plays it and then everything after it. Alt+Shift+Page Down and Alt+Shift+Page Up move to the next and previous audio in that queue.

You don't have to stay on the page an audio came from. The player keeps going while you look around, and the Player menu works from anywhere in the window:

- Ctrl+P plays or pauses, and Ctrl+Period stops.
- Alt+Shift+Left and Alt+Shift+Right skip back and forward. How far is a setting; the menu shows the current amount.
- Alt+Shift+Up and Alt+Shift+Down change the volume.
- Alt+Period and Alt+Comma make it faster or slower, from half speed to double, without changing the pitch of anyone's voice. Player, Speed lists the speeds. The speed you choose stays for everything you play.
- Ctrl+Shift+W tells you what's playing, who posted it, and how far through you are.

In the player at the bottom of the window, the Position slider moves through the audio: the arrow keys move 5 seconds, Page Up and Page Down move 30, Home and End go to the start and end, and the number keys jump through it, with 5 halfway.

Save audio, on an audio's menu or under More on its page, keeps a copy of the original file on your computer. Pubseed remembers the folder you saved to last.

## An audio's page and its comments

Press Enter on an audio to open its page. At the top are its description and the Play, Favorite and More buttons. Under More you'll find:

- Subscribe to, or unsubscribe from, whoever posted it.
- Comment notifications, on or off. When they're on, AudioPub tells you whenever someone comments on it.
- View profile, Copy link, Save audio, and Open in your browser.
- Edit and Delete, when it's yours.

Below that are the comments, with replies under the comment they answer and marked "Reply". On a reply, Go to the comment it replies to takes you back up to it. Press Enter on a comment to reply to it, or write a new comment in the box under the list and press Enter to post it. Shift+Enter starts a new line in a comment.

When a comment or a chat message has a web link in it, its menu has Open link. Pubseed shows you the whole address before opening it in your browser, so you can see where it goes first.

## Posting audio

File, New audio (Ctrl+N) opens a window for a new post. It's a separate window, so you can keep using Pubseed while it's open, and Ctrl+N brings it back to the front.

Under "Audio from", choose whether to record it now or use a file on your computer.

- To record, press Record. Your default microphone is used. Pause if you need a moment, and press Stop recording when you're done. Play back lets you hear it, and Discard throws it away so you can start again. Anything shorter than two seconds isn't kept.
- To use a file, press Choose file. It can be in any format AudioPub takes, up to 500 MB.

Give it a title of 3 to 120 characters, and a description if you like, then press Publish. The Audio field shows how the upload is going. When it's done, the window closes, your feeds are brought up to date, and the new audio's page opens. Closing the window while it has something in it asks before throwing it away.

To change the title or description of something you've posted, use Edit on its menu or under More on its page. AudioPub allows three edits to each post. Delete removes it for good, after asking.

## Live streams

The Live tab lists who's streaming right now. Press Enter on a stream to join it. Its audio starts, and the tab turns into the stream's room: a short description, the chat, and a box to write in. Type a message and press Enter to send it.

New chat messages are read out as they arrive. Ctrl+Shift+R turns that on and off. You can move to other tabs and keep listening; the room stays where it is until you leave.

- Ctrl+P stops the stream's audio while you stay in the chat, and starts it again.
- Ctrl+Shift+S tells you how many people are listening.
- Ctrl+Shift+L leaves the stream.

Pubseed can't broadcast a stream of your own yet. For that, have a look at [Pubsplash](https://github.com/cha0t1cnu3tral/pubsplash).

## Notifications

The Notifications tab lists comments, replies and anything else AudioPub has told you about. Opening the tab marks them as read. Press Enter on one to go to what it's about, and Delete to remove it. Clear all, on a notification's menu, removes every one of them after asking.

Pubseed checks for new notifications every minute or so and updates the count on the tab.

## Settings

File, Settings (Ctrl+Comma) has:

- **Appearance:** light, dark, or the same as Windows. This takes effect the next time you start Pubseed.
- **Skip back and forward by:** how far the skip keys move.
- **Read live chat messages aloud:** the same as Ctrl+Shift+R.
- **Check for updates when Pubseed starts:** on unless you turn it off. Help, Check for updates works either way.
- **Read chat with:** your screen reader, or one of the voices installed in Windows. A separate voice keeps chat from interrupting your screen reader. If you choose a voice, you can pick which one, set how fast it talks, and try it out with Test the voice.

Settings are kept in `%APPDATA%\pubseed\settings.json`.

## Global hotkeys

Global hotkeys work while another program is in front, so you can control Pubseed without switching to it. File, Global hotkeys lists them. Choose one and press Set to give it a key, Clear to turn it off, or Reset to give it back its usual key. A global hotkey needs at least one of Ctrl, Alt or the Windows key, so it doesn't take a key other programs need.

| Hotkey | Usual key | On to begin with |
| --- | --- | --- |
| Show or hide Pubseed | Ctrl+Alt+Shift+A | Yes |
| Play or pause | Ctrl+Alt+Shift+P | No |
| Next in queue | Ctrl+Alt+Shift+Right | No |
| Previous in queue | Ctrl+Alt+Shift+Left | No |
| Read the last announcement | Ctrl+Alt+Shift+L | No |

Show or hide Pubseed hides the window when it's in front, and brings it back otherwise. While it's hidden, anything playing keeps going, and Pubseed's icon in the notification area brings the window back too. Its menu has Restore and Exit.

If another program already uses a hotkey's key, Pubseed tells you, and that hotkey won't work until you give it a different key.

## Keyboard shortcuts

Every shortcut is also shown on the menu next to its command.

### File

| Keys | Command |
| --- | --- |
| Ctrl+N | New audio |
| Ctrl+Comma | Settings |
| Ctrl+Q | Exit |

### View

| Keys | Command |
| --- | --- |
| Ctrl+1 to Ctrl+4 | Home, Subscriptions, Live, Notifications |
| F5 | Refresh |

### Go

| Keys | Command |
| --- | --- |
| Escape or Alt+Left | Back |
| Ctrl+U | Your profile |
| Ctrl+B | Favorites |
| Ctrl+F | Search |
| F6 and Shift+F6 | Next and previous region |

### Player

| Keys | Command |
| --- | --- |
| Ctrl+P | Play or pause |
| Ctrl+Period | Stop |
| Alt+Shift+Left and Alt+Shift+Right | Skip back and forward |
| Alt+Shift+Page Up and Alt+Shift+Page Down | Previous and next in the queue |
| Alt+Shift+Up and Alt+Shift+Down | Volume up and down |
| Alt+Period and Alt+Comma | Faster and slower |
| Ctrl+Shift+W | What's playing |

### Live

| Keys | Command |
| --- | --- |
| Ctrl+Shift+R | Read chat aloud on or off |
| Ctrl+Shift+S | Who's listening |
| Ctrl+Shift+L | Leave the stream |

### Help

| Keys | Command |
| --- | --- |
| F1 | Pubseed help |

### In lists

| Keys | Command |
| --- | --- |
| Enter | Open, join, or reply |
| Space | Play or pause |
| Left and Right | Skip back and forward |
| Delete | Delete your comment or audio, or a notification |
| Applications or Shift+F10 | Everything else for that row |

## Getting help

If something doesn't work the way this page says, or you have an idea, open an issue on [Pubseed's GitHub page](https://github.com/seediffusion/pubseed/issues). Saying what you did, what you expected and what happened instead makes it much easier to fix.

AudioPub doesn't offer an official way for other programs to talk to it, so Pubseed uses the same pages the website does. If the site changes, parts of Pubseed can stop working until they're updated.
