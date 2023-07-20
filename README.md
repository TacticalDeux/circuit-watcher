# circuit-watcher

Auto-accept LoL (League of Legends) queue (any).  
Small project I've been working on (doesn't even have an icon yet) and plan to update and work on more features, although nothing too crazy.  
There's also probably a whole lot of better ways to do the stuff I'm doing but there's not much to base the project off of and even less in rust.  

## BUILDING/DOWNLOADING

Building currently not possible as I'm using a locally modified version of the rust-lcu-connector crate. Will be updated later.
```sh
git clone https://github.com/TacticalDeuce/circuit-watcher.git
cd circuit-watcher
```

~~And download [this fork](https://github.com/sona-voice/rust-lcu-connector) (that fixes the terminal popups) of [league-client-connector](https://crates.io/crates/league-client-connector) and edit the line that says `league-client-connector = { path="" }` to whichever directory the downloaded fork is stored in. It would look something like this:~~

```toml
league-client-connector = { path="C:/{User}/Documents/rust-lcu-connector-master" }
```

~~You can change the name of the master folder to whatever you want.~~

Or download through the [release page](https://github.com/TacticalDeuce/circuit-watcher/releases), extracting the folder on your desktop (or somewhere) and running the .exe.

## Features

- Queue auto-accept.
- Toggeable auto-pick and auto-ban.
- Version checking and downloading from the GUI.
- Auto summoner spell selection. Will check assigned role and spell selection and,  
  if the role is jungle and smite is not selected yet, change whichever spell that is neither ghost nor flash to smite.  
  If both slots are ghost and flash (or vice versa) it will default smite to the first slot.
- ~~Toggeable rune page change (based on auto-pick). Only works on non-recommended rune pages, will delete rune page if it's data is auto-recommended, can be bypassed by using auto-recommendation after locking champ.~~ TODO

### TODO

- [X] Queue auto-accept
- [X] Terminal timestamps (mostly for debugging purposes)
- [X] Champ auto-pick
- [X] Champ auto-ban
- [X] Auto summoner spell selection
- [X] GUI
- [ ] Persistent settings
- [ ] Pick runes depending on champ auto-locked (might reduce pick alternatives to only one instead of two)
- [ ] Role check when auto-picking so champs aren't locked/banned if you didn't get main role
- [ ] Maybe queue rejoining?

***

Using [eframe/egui](https://github.com/emilk/egui) for all the GUI stuff.  
Using [rust-lcu-connector (league-client-connector fork)](https://github.com/sona-voice/rust-lcu-connector) for getting the port and auth password the LeagueClient uses for HTTP requests.

***

This is not an official Riot Games product. It's not affiliated with or endorsed by Riot Games Inc.
