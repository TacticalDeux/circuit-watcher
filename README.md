# circuit-watcher

Auto-accept LoL (League of Legends) queue (any).  
Small project I've been working on (doesn't even have an icon yet) and plan to update and work on more features, although nothing too crazy.  
There's also probably a whole lot of better ways to do the stuff I'm doing but there's not much to base the project off of and even less in rust.

## BUILDING/DOWNLOADING

Should be as easy as doing

```sh
git clone https://github.com/TacticalDeuce/circuit-watcher.git
cd circuit-watcher
cargo build --release
```

or downloading through the [release page](https://github.com/TacticalDeuce/circuit-watcher/releases)

## Features

- Queue auto-accept.
- Toggeable auto-pick and auto-ban.
- Toggeable rune page change (based on auto-pick). Only works on non-recommended rune pages, will delete rune page if it's data is auto-recommended, can be bypassed by using auto-recommendation after locking champ.

### TODO

- [x] Queue auto-accept
- [x] Terminal timestamps (mostly for debugging purposes)
- [x] Champ auto-pick
- [x] Champ auto-ban
- [x] Pick runes depending on champ auto-locked (might reduce pick alternatives to only one instead of two)
- [ ] Role check when auto-picking so champs aren't locked/banned if you didn't get main role
- [ ] Maybe queue rejoining?
- [ ] Maybe an actual UI (veering off of CLI)?

***

This is not an official Riot Games product. It's not affiliated with or endorsed by Riot Games Inc.
