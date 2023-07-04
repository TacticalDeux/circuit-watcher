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
            println!("{} {}", formatted_time, format_args!($($arg)*));
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
/// The `Champion` struct is a data structure used for (de)serialization of the `champsions.json` file.
///
/// ### Properties:
/// * `id`: The `id` property is of type `u32`, which stands for "unsigned 32-bit integer". It is used
/// to uniquely identify each instance of the `Champion` struct.
/// * `name`: The `name` property is a string that represents the name of a champion.
struct Champion {
    id: u32,
    name: String,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize, Clone)]
/// The `ActionResponseData` struct is a data structure used to represent the response data for a champion select action.
///
/// ### Properties:
/// * `actorCellId`: The `actorCellId` property is of type `i32`, which stands for a 32-bit signed
/// integer. It represents the ID of a summoner in the given champion selection lobby.
/// * `completed`: The "completed" property is a boolean value that indicates whether the action
/// associated with the response data has been completed or not.
/// * `id`: The `id` property is of type `i32`, which stands for a 32-bit signed integer. It is used to
/// uniquely identify an action response data object. It differs from the `actorCellId` by being a unique id tied to the action `r#type`.
/// * `isInProgress`: The `isInProgress` property is a boolean value that indicates whether the action
/// is currently in progress or not.
/// * `r#type`: The property "r#type" is a string that represents the type of action response data. The
/// "r#" prefix is used to escape the reserved keyword "type" in Rust.
struct ActionResponseData {
    actorCellId: i32,
    completed: bool,
    id: i32,
    isInProgress: bool,
    r#type: String,
}

#[allow(non_snake_case)]
#[derive(Deserialize, Serialize)]
/// The `RunesData` struct represents the rune data needed to do a POST request to create a new rune page.
/// There is more data in a GET request response for runes but only this fields are needed for the creation of a page.
///
/// ### Properties:
/// * `id`: The `id` property is of type `u64`, which stands for unsigned 64-bit integer. It is used to
/// uniquely identify each `RunesData` object.
/// * `name`: The `name` property is a string that represents the name of the rune page.
/// * `primaryStyleId`: The `primaryStyleId` property represents the ID of the primary rune style chosen
/// for a particular rune set (E.g. Domination).
/// * `selectedPerkIds`: The `selectedPerkIds` property is a vector (dynamic array) of `u32` values. It
/// is used to store the IDs of the selected perks for a particular rune (E.g. Cheap Shot, Eyeball Collection and Relentless Hunter).
/// * `subStyleId`: The `subStyleId` property in the `RunesData` struct represents the ID of the secondary rune.
struct RunesData {
    id: u64,
    name: String,
    primaryStyleId: u32,
    selectedPerkIds: Vec<u32>,
    subStyleId: u32,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let process_pattern = "LeagueClient";
    let mut system = System::new_all();
    let pick_ban_selection = Arc::new(AtomicBool::new(true));
    let rune_page_selection = Arc::new(AtomicBool::new(false));
    let _program_control_thread = tokio::spawn(key_listener(
        pick_ban_selection.clone(),
        rune_page_selection.clone(),
    ));

    println!("Press the END key to terminate the program.");
    println!("Press LSHIFT + HOME keys to toggle rune page change (based on auto-pick selection).");
    println!("Press the HOME key to choose auto-pick and auto-ban champions.\nPress it again to clear your picks and turn off auto-pick/ban");
    while !(find_processes_by_regex(&mut system, process_pattern).await) {
        println!("The pattern '{process_pattern}' does not match any active processes. You may have closed the LoL client.\nRetrying in 10 seconds.");
        tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
    }
    println!(
        "League process found. Waiting for 10 seconds to ensure connectivity to league client."
    );
    println!(
        "If an error is thrown try reopening the program when the LeagueClient is fully opened."
    );
    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
    // sleeping for if the client process is running but the UX hasn't yet spawned
    // so no lockfile would be generated, thus giving an error.

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
    let mut champ_pick_ids: Vec<(u32, String)> = Vec::new();
    let mut champ_ban_id: Option<(u32, String)> = None;
    let mut locked_champ = false;
    let mut champ_pick_magic_number = 1; // this is just to check which champion has ben picked/banned on the champion_pick_ids Vec.
                                              // named it this way just for funsies, no real reason lol. Is an int because Vec used to be at least of length 3.
    loop {
        let mut no_input = false;
        let rune_data: serde_json::Value = rest_client
            .get(format!(
                "https://127.0.0.1:{}/lol-perks/v1/pages",
                lockfile.port
            ))
            .send()
            .await?
            .json()
            .await?;
        let rune_data_respone: Vec<RunesData> = serde_json::from_value(rune_data.clone())?;
        let extracted_rune_data: Vec<(u64, String, u32, Vec<u32>, u32)> = rune_data_respone
            .iter()
            .map(|data| {
                (
                    data.id,
                    data.name.clone(),
                    data.primaryStyleId,
                    data.selectedPerkIds.clone(),
                    data.subStyleId,
                )
            })
            .collect();
        let (
            rune1_id,
            rune1_name,
            rune1_primary_style_id,
            rune1_selected_perk_ids,
            rune1_substyle_id,
        ) = extracted_rune_data.get(0).cloned().unwrap();
        let (
            rune2_id,
            rune2_name,
            rune2_primary_style_id,
            rune2_selected_perk_ids,
            rune2_substyle_id,
        ) = extracted_rune_data
            .get(1)
            .cloned()
            .unwrap_or((0, "".to_string(), 0, vec![0], 0));

        let already_picked = pick_ban_selection.load(Ordering::SeqCst);
        if !already_picked {
            if champ_pick_ids.len() != 0 || champ_ban_id.is_some() {
                champ_ban_id = None;
                champ_pick_ids.clear();

                println!(
                    "Picks and ban selection cleared, turning auto-pick/ban off, press home to pick your champions again."
                );
                pick_ban_selection.store(true, Ordering::SeqCst);
                continue;
            }
            for i in 0..3 {
                let mut valid_name_entered = false;
                let mut input = String::new();

                if i == 0 {
                    println!("Enter a champion name to pick (press enter to skip):");
                }
                if i == 1 {
                    if no_input {
                        continue;
                    }
                    println!("Enter an alternative champion name to pick (press enter to skip):");
                }
                if i == 2 {
                    println!("Enter a champion name to ban (press enter to skip):");
                }

                while !valid_name_entered {
                    io::stdin()
                        .read_line(&mut input)
                        .expect("Failed to read input");

                    input = input.trim().to_string().to_lowercase();

                    if i == 2 && no_input && input.is_empty() {
                        println!("Empty string entered. There has to be at least one champion either to ban or pick.");
                        input.clear();
                        continue;
                    }

                    if i == 0 && input.is_empty() {
                        no_input = true;
                        valid_name_entered = true;
                    }
                    if i == 1 && input.is_empty() {
                        no_input = false;
                        valid_name_entered = true;
                    }
                    if i == 2 && !no_input && input.is_empty() {
                        valid_name_entered = true;
                    }

                    if !input.is_empty() {
                        let matching_champion = champions
                            .iter()
                            .find(|champion| champion.name.to_lowercase() == input);

                        match matching_champion {
                            Some(champion) => {
                                if champ_pick_ids.contains(&(champion.id, champion.name.clone())) {
                                    println!(
                                        "Champion already selected. Please choose a different one."
                                    );
                                    input.clear();
                                    continue;
                                }
                                if i == 2 {
                                    champ_ban_id = Some((champion.id, champion.name.clone()));
                                } else {
                                    champ_pick_ids.push((champion.id, champion.name.clone()));
                                }
                                valid_name_entered = true;
                            }
                            None => {
                                println!("No matching champion found. Please try again.");
                                input.clear();
                            }
                        }
                    }
                }
            }
            pick_ban_selection.store(true, Ordering::SeqCst);
            if champ_ban_id.is_some() {
                println!(
                    "Champion to ban: {} id:{}",
                    &champ_ban_id.as_ref().unwrap().1,
                    &champ_ban_id.as_ref().unwrap().0
                );
            }
            if champ_pick_ids.len() != 0 {
                for (id, name) in &champ_pick_ids {
                    println!("Champion to pick: {}, id:{}", name, id);
                }

                println!(
                    "Rune page \"{}\" for {} (only if rune page change is active)",
                    &rune1_name, &champ_pick_ids[0].1 /*first champ's name*/
                );
                if champ_pick_ids.len() > 1 {
                    println!(
                        "Rune page \"{}\" for {}",
                        &rune2_name, &champ_pick_ids[1].1 /*second champ's name*/
                    );
                }
            }
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
                champ_pick_magic_number = 1;
                found_match = false;
                locked_champ = false;
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
                let rune_change = rune_page_selection.load(Ordering::SeqCst);

                if champ_pick_ids.len() == 0 && champ_ban_id.is_none() {
                    continue;
                }

                let current_champ_select: serde_json::Value = rest_client
                    .get(format!(
                        "https://127.0.0.1:{}/lol-champ-select/v1/session",
                        lockfile.port
                    ))
                    .send()
                    .await?
                    .json()
                    .await?;

                let action_response: Vec<Vec<ActionResponseData>> =
                    serde_json::from_value(current_champ_select["actions"].clone())?;
                let filtered_action_data: Vec<ActionResponseData> = action_response
                    .iter()
                    .flatten()
                    .filter(|data| data.actorCellId == current_champ_select["localPlayerCellId"])
                    .take(2) // Limit to a maximum of 2 matches
                    .cloned()
                    .collect();
                let extracted_action_data: Vec<(i32, bool, String, bool)> = filtered_action_data
                    .iter()
                    .map(|data| {
                        (
                            data.id,
                            data.isInProgress,
                            data.r#type.clone(),
                            data.completed,
                        )
                    })
                    .collect();

                let (ban_id, ban_is_in_progress, _type1, ban_completed) = extracted_action_data
                    .get(0)
                    .cloned()
                    .unwrap_or((0, false, "".to_string(), false));
                let (pick_id, pick_is_in_progress, _type2, pick_completed) = extracted_action_data
                    .get(1)
                    .cloned()
                    .unwrap_or((0, false, "".to_string(), false));

                if champ_ban_id.is_some() {
                    let ban_body = serde_json::json!({
                            "actorCellId": current_champ_select["localPlayerCellId"],
                            "championId": &champ_ban_id.as_ref().unwrap().0,
                            "completed": true,
                            "id": &ban_id,
                            "isAllyAction": true,
                            "type": "ban"
                    });
                    let ban_champ_info: serde_json::Value = rest_client
                        .get(format!(
                            "https://127.0.0.1:{}/lol-champ-select/v1/grid-champions/{}",
                            lockfile.port,
                            &champ_ban_id.as_ref().unwrap().0
                        ))
                        .send()
                        .await?
                        .json()
                        .await?;

                    if ban_is_in_progress
                        && !ban_completed
                        && ban_champ_info["selectionStatus"]["pickedByOtherOrBanned"] != true
                        && current_champ_select["timer"]["phase"] != "PLANNING"
                    {
                        rest_client
                            .patch(format!(
                                "https://127.0.0.1:{}/lol-champ-select/v1/session/actions/{}",
                                lockfile.port, ban_id
                            ))
                            .json(&ban_body)
                            .send()
                            .await?;
                        timestamped_println!(
                            "Banned champion {} id:{}",
                            &champ_ban_id.as_ref().unwrap().1,
                            &champ_ban_id.as_ref().unwrap().0
                        );
                        tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                    }
                }

                if champ_pick_ids.len() != 0 {
                    for (champ_pick_id, champ_pick_name) in &champ_pick_ids {
                        let pick_champ_info: serde_json::Value = rest_client
                            .get(format!(
                                "https://127.0.0.1:{}/lol-champ-select/v1/grid-champions/{}",
                                lockfile.port, champ_pick_id
                            ))
                            .send()
                            .await?
                            .json()
                            .await?;

                        let pick_body = serde_json::json!({
                                "actorCellId": current_champ_select["localPlayerCellId"],
                                "championId": champ_pick_id,
                                "completed": true,
                                "id": &pick_id,
                                "isAllyAction": true,
                                "type": "pick"
                        });
                        let rune1_body = serde_json::json!({
                            "name": rune1_name.clone(),
                            "primaryStyleId": rune1_primary_style_id,
                            "selectedPerkIds": rune1_selected_perk_ids.clone(),
                            "subStyleId": rune1_substyle_id
                        });
                        let rune2_body = serde_json::json!({
                            "name": rune2_name.clone(),
                            "primaryStyleId": rune2_primary_style_id,
                            "selectedPerkIds": rune2_selected_perk_ids.clone(),
                            "subStyleId": rune2_substyle_id
                        });

                        if !pick_is_in_progress
                            && pick_completed
                            && !ban_is_in_progress
                            && ban_completed
                            || current_champ_select["timer"]["phase"] == "PLANNING"
                        {
                            break;
                        }

                        if pick_champ_info["selectionStatus"]["pickedByOtherOrBanned"] == true {
                            champ_pick_magic_number += 1;
                            break;
                        }

                        if !pick_is_in_progress {
                            break;
                        }
                        if pick_is_in_progress
                            && !pick_completed
                            && !ban_is_in_progress
                            && ban_completed
                            && pick_champ_info["selectionStatus"]["pickedByOtherOrBanned"] != true
                            && !locked_champ
                        {
                            if rune_change {
                                if champ_pick_magic_number == 1 {
                                    rest_client
                                        .delete(format!(
                                            "https://127.0.0.1:{}/lol-perks/v1/pages/{}",
                                            lockfile.port, rune1_id
                                        ))
                                        .send()
                                        .await?;

                                    tokio::time::sleep(tokio::time::Duration::from_millis(200))
                                        .await;

                                    rest_client
                                        .post(format!(
                                            "https://127.0.0.1:{}/lol-perks/v1/pages",
                                            lockfile.port
                                        ))
                                        .json(&rune1_body)
                                        .send()
                                        .await?;
                                } else {
                                    rest_client
                                        .delete(format!(
                                            "https://127.0.0.1:{}/lol-perks/v1/pages/{}",
                                            lockfile.port, rune2_id
                                        ))
                                        .send()
                                        .await?;

                                    tokio::time::sleep(tokio::time::Duration::from_millis(200))
                                        .await;

                                    rest_client
                                        .post(format!(
                                            "https://127.0.0.1:{}/lol-perks/v1/pages",
                                            lockfile.port
                                        ))
                                        .json(&rune2_body)
                                        .send()
                                        .await?;
                                }
                            }
                            rest_client
                                .patch(format!(
                                    "https://127.0.0.1:{}/lol-champ-select/v1/session/actions/{}",
                                    lockfile.port, pick_id
                                ))
                                .json(&pick_body)
                                .send()
                                .await?;
                            timestamped_println!(
                                "Picked champion {} id:{}",
                                champ_pick_name,
                                champ_pick_id
                            );
                            locked_champ = true;
                            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                        }
                        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                    }
                }
            }
            Some("InProgress") => {
                timestamped_println!("Game in progress...");
                tokio::time::sleep(tokio::time::Duration::from_secs(20)).await;
            }
            Some("WaitingForStats") => {
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
            Some("PreEndOfGame") => {
                timestamped_println!("Game in progress...");
                tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
            }
            Some("EndOfGame") => {
                timestamped_println!("Game ending...");
                tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
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
}

/// This Rust function searches for processes matching a given regular expression pattern and returns a
/// boolean indicating whether any matching processes were found.
///
/// ### Arguments:
///
/// * `system`: A mutable reference to a `System` struct, which represents the current system state and
/// provides methods for interacting with system resources such as processes, memory, and CPU usage.
/// * `process_pattern`: A string pattern that is used to match against the names of processes running
/// on the system.
///
/// #### Returns:
///
/// A boolean value indicating whether there are any processes in the system that match the given
/// regular expression pattern.
///
/// The function will return true if at least one process name matches the pattern, and
/// false otherwise.
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

/// The function `key_listener` listens for key presses and terminates the program if the END key is
/// pressed, or sets a flag to false if the HOME key is pressed.
///
/// ### Arguments:
///
/// * `pick_ban_selection`: The `pick_ban_selection` parameter is an `Arc<AtomicBool>` which is a
/// thread-safe atomic boolean value wrapped in an `Arc` (atomic reference count) smart pointer. It is
/// used to control the pick and ban selection process in the program.
async fn key_listener(pick_ban_selection: Arc<AtomicBool>, rune_page_selection: Arc<AtomicBool>) {
    use winapi::um::winuser::GetAsyncKeyState;
    let pressed = -32767;
    loop {
        let end_key = unsafe {
            GetAsyncKeyState(0x23 /*VK_END*/)
        };
        let home_key = unsafe {
            GetAsyncKeyState(0x24 /*VK_HOME*/)
        };
        let left_shift_key = unsafe {
            GetAsyncKeyState(0xA0 /*VK_LSHIFT*/)
        };

        if end_key == pressed {
            println!("\nEND key pressed, terminating the program in 5 seconds.");
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            std::process::exit(0);
        }

        if home_key == pressed && left_shift_key == -32768
        /*pressed*/
        {
            let rune_page_selection_val = rune_page_selection.load(Ordering::SeqCst);
            rune_page_selection.store(!rune_page_selection_val, Ordering::SeqCst);
            println!(
                "Rune page change is now set to: {}",
                !rune_page_selection_val
            );
            continue;
        }

        if home_key == pressed {
            pick_ban_selection.store(false, Ordering::SeqCst);
            println!("\nHome key pressed.");
        }
    }
}
