use chrono::prelude::*;
use http::{header::AUTHORIZATION, HeaderValue};
use league_client_connector::LeagueClientConnector;
use regex::Regex;
use reqwest::{header, ClientBuilder};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::io::{self, Write};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use sysinfo::{self, ProcessExt, System, SystemExt};

macro_rules! timestamped_println {
    ($($arg:tt)*) => {
        {
            let current_time = Local::now();
            let formatted_time = current_time.format("[%Y-%m-%d - %H:%M]");
            println!("{}: {}", formatted_time, format_args!($($arg)*));
        }
    }
}
macro_rules! timestamped_print {
    ($($arg:tt)*) => {
        {
            let current_time = Local::now();
            let formatted_time = current_time.format("[%Y-%m-%d - %H:%M]");
            print!("{} {}", formatted_time, format_args!($($arg)*));
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct Champion {
    id: u32,
    name: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let process_pattern = "LeagueClient";
    let mut system = System::new_all();
    let pick_ban_selection = Arc::new(AtomicBool::new(true));
    let program_control_thread = tokio::spawn(key_listener(pick_ban_selection.clone()));

    println!("Press the END key to terminate the program.");
    println!("Press the HOME key to choose auto-pick and auto-ban champions.");
    while !(find_processes_by_regex(&mut system, process_pattern).await) {
        if program_control_thread.is_finished() {
            return Ok(());
        }
        println!("The pattern '{process_pattern}' does not match any active processes. You may have closed the LoL client.\nRetrying in 30 seconds.");
        tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
    }

    let lockfile = LeagueClientConnector::parse_lockfile().unwrap();
    let auth_header =
        HeaderValue::from_str(format!("Basic {}", lockfile.b64_auth).as_str()).unwrap();
    timestamped_println!(
        "Connected to LoL client at {}:{} with auth: base64:{}\n",
        lockfile.address,
        lockfile.port,
        lockfile.b64_auth
    );
    let cert = reqwest::Certificate::from_pem(include_bytes!("../riotgames.pem")).unwrap();
    let mut headers = header::HeaderMap::new();

    headers.insert(AUTHORIZATION, auth_header.clone());
    let rest_client = ClientBuilder::new()
        .add_root_certificate(cert)
        .default_headers(headers)
        .build()
        .unwrap();

    let json_data = std::fs::read_to_string("champions.json").expect("Failed to read file");
    let champions: Vec<Champion> = serde_json::from_str(&json_data).expect("Failed to parse JSON");

    let mut dots_count = 0;
    let mut found_match = false;
    let mut dodge_check = true;
    let mut champ_pick_ids: Vec<(u32, String)> = Vec::new();
    let mut champ_ban_id: Option<(u32, String)> = None;
    loop {
        let already_picked = pick_ban_selection.load(Ordering::SeqCst);
        if !already_picked {
            if program_control_thread.is_finished() {
                return Ok(());
            }
            if champ_pick_ids.len() >= 3 {
                let mut input = String::new();
                champ_pick_ids.clear();

                println!("Press enter with no inputs to exit auto-pick/ban selection");
                println!("Enter a champion name to pick");

                io::stdin()
                    .read_line(&mut input)
                    .expect("Failed to read input");
                input = input.trim().to_string().to_lowercase();

                if input.is_empty() {
                    pick_ban_selection.store(true, Ordering::SeqCst);
                    input.clear();
                    continue;
                }
            }
            for i in 0..4 {
                if program_control_thread.is_finished() {
                    return Ok(());
                }
                let mut valid_name_entered = false;
                let mut input = String::new();

                if i == 0 {
                    println!("Enter a champion name to pick:");
                }
                if i == 1 {
                    println!("Enter an alternative champion name to pick:");
                }
                if i == 2 {
                    println!("Enter an alternative champion name to pick:");
                }
                if i == 3 {
                    println!("Enter a champion name to ban:");
                }

                while !valid_name_entered {
                    if program_control_thread.is_finished() {
                        return Ok(());
                    }
                    io::stdin()
                        .read_line(&mut input)
                        .expect("Failed to read input");

                    input = input.trim().to_string().to_lowercase();

                    if !input.is_empty() {
                        let matching_champion = champions
                            .iter()
                            .find(|champion| champion.name.to_lowercase() == input);

                        match matching_champion {
                            Some(champion) => {
                                if i == 3 {
                                    champ_ban_id = Some((champion.id, champion.name.clone()));
                                } else {
                                    if champ_pick_ids
                                        .contains(&(champion.id, champion.name.clone()))
                                    {
                                        println!(
                                        "Champion already selected. Please choose a different one."
                                    );
                                        input.clear();
                                        continue;
                                    }
                                    champ_pick_ids.push((champion.id, champion.name.clone()));
                                }
                                valid_name_entered = true;
                            }
                            None => {
                                println!("No matching champion found. Please try again.");
                                input.clear();
                            }
                        }
                    } else {
                        println!("Empty string entered. Please try again.");
                    }
                }
            }
            pick_ban_selection.store(true, Ordering::SeqCst);
            for (id, name) in &champ_pick_ids {
                println!("Champions (and alternatives) to pick: {} id:{}", name, id);
            }
            println!(
                "Champion to ban: {} id:{}",
                champ_ban_id.clone().unwrap().1,
                champ_ban_id.clone().unwrap().0
            );
        }

        let gameflow: serde_json::Value = rest_client
            .get(format!(
                "https://127.0.0.1:{}/lol-gameflow/v1/session",
                lockfile.port
            ))
            .send()
            .await?
            .json()
            .await?;
        let phase = gameflow["phase"].as_str();

        if program_control_thread.is_finished() {
            break;
        }

        match phase {
            Some("Matchmaking") => {
                dots_count = (dots_count + 1) % 4;
                print!("\r");
                timestamped_print!("Searching for a match");
                for _ in 0..dots_count {
                    print!(".");
                }
                for _ in dots_count..3 {
                    print!(" ");
                }
                std::io::stdout().flush().unwrap();
                found_match = false;
                dodge_check = true;
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            }
            Some("Lobby") => {
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            }
            Some("ReadyCheck") => {
                if !found_match {
                    found_match = true;
                    print!("\n");
                    timestamped_println!("Match found, accepting.");
                    rest_client
                        .post(format!(
                            "https://127.0.0.1:{}/lol-matchmaking/v1/ready-check/accept",
                            lockfile.port
                        ))
                        .send()
                        .await?;
                }
            }
            Some("ChampSelect") => {
                if champ_pick_ids.len() < 3 {
                    continue;
                }
                for (pick_id, pick_name) in &champ_pick_ids {
                    let mut action_id = 0;
                    let current_champ_select: serde_json::Value = rest_client
                        .get(format!(
                            "https://127.0.0.1:{}/lol-champ-select/v1/session",
                            lockfile.port
                        ))
                        .send()
                        .await?
                        .json()
                        .await?;
                    let ban_champ_info: serde_json::Value = rest_client
                        .get(format!(
                            "https://127.0.0.1:{}/lol-champ-select/v1/grid-champions/{}",
                            lockfile.port,
                            champ_ban_id.clone().unwrap().0
                        ))
                        .send()
                        .await?
                        .json()
                        .await?;
                    let pick_champ_info: serde_json::Value = rest_client
                        .get(format!(
                            "https://127.0.0.1:{}/lol-champ-select/v1/grid-champions/{}",
                            lockfile.port, pick_id
                        ))
                        .send()
                        .await?
                        .json()
                        .await?;

                    if dodge_check {
                        dodge_check = false;
                        action_id = 1;
                    }

                    if let Some(arr) = current_champ_select["actions"].as_array() {
                        for _ in arr {
                            action_id += 1;
                        }
                    }

                    let ban_body = serde_json::json!({
                            "actorCellId": current_champ_select["localPlayerCellId"],
                            "championId": champ_ban_id.clone().unwrap().0,
                            "completed": true,
                            "id": action_id,
                            "isAllyAction": true,
                            "type": "ban"
                    });
                    let pick_body = serde_json::json!({
                            "actorCellId": current_champ_select["localPlayerCellId"],
                            "championId": pick_id,
                            "completed": true,
                            "id": action_id,
                            "isAllyAction": true,
                            "type": "pick"
                    });
                    // adding [num][0] to unwrap the real values because ["actions"] returns [[{content}]]
                    // each new action is a new object callable only by index (num)
                    if current_champ_select["actions"][action_id - 1][0]["actorCellId"]
                        == current_champ_select["localPlayerCellId"]
                        && current_champ_select["actions"][action_id - 1][0]["isAllyAction"] == true
                    {
                        if current_champ_select["actions"][action_id - 1][0]["type"] == "ban" {
                            if ban_champ_info["selectionStatus"]["pickedByOtherOrBanned"] != true {
                                rest_client
                                    .patch(format!(
                                    "https://127.0.0.1:{}/lol-champ-select/v1/session/actions/{}",
                                    lockfile.port, action_id
                                ))
                                    .json(&ban_body)
                                    .send()
                                    .await?;
                                println!(
                                    "Banned {} id {}",
                                    champ_ban_id.clone().unwrap().1,
                                    champ_ban_id.clone().unwrap().0
                                );
                                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                            }
                        }
                        if current_champ_select["actions"][action_id - 1][0]["type"] == "pick" {
                            if pick_champ_info["selectionStatus"]["pickedByOtherOrBanned"] == true {
                                continue;
                            }
                            rest_client
                                .patch(format!(
                                    "https://127.0.0.1:{}/lol-champ-select/v1/session/actions/{}",
                                    lockfile.port, action_id
                                ))
                                .json(&pick_body)
                                .send()
                                .await?;
                            println!("Picked champion {}", pick_name);
                            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                        }
                    }
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                }
            }
            Some(unimplemented_phase) => {
                println!("Unimplemented: {}", unimplemented_phase);
            }
            None => {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    Ok(())
}

async fn find_processes_by_regex(system: &mut System, process_pattern: &str) -> bool {
    system.refresh_all();
    let regex = Regex::new(process_pattern).unwrap();
    let mut process_exists = false;

    for process in system.processes().values() {
        if regex.is_match(process.name()) {
            process_exists = true;
        }
    }
    process_exists
}

async fn key_listener(pick_ban_selection: Arc<AtomicBool>) {
    use winapi::um::winuser::GetAsyncKeyState;
    let pressed = -32767;
    loop {
        let end_key = unsafe {
            GetAsyncKeyState(0x23 /*VK_END*/)
        };
        let home_key = unsafe {
            GetAsyncKeyState(0x24 /*VK_HOME*/)
        };

        if end_key == pressed {
            println!("\nEND key pressed, terminating the program.");
            break;
        }

        if home_key == pressed {
            pick_ban_selection.store(false, Ordering::SeqCst);
            println!("Home key pressed.");
        }
    }
}
