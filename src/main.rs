use std::convert::TryFrom;
use matrix_sdk::{
    BaseRoom, Client, SyncSettings, Result, room::Room, room::Common,
    ruma::{UserId, events::{SyncMessageEvent, AnyMessageEventContent, room::message::MessageEventContent, room::message::MessageType}},
};
use matrix_sdk_common::uuid::Uuid;
use std::{thread, time, env, process};
use regex::Regex;

mod bgchan;
mod db;
mod grocery;

const ENV_VAR_HOMECHATBOT_USERNAME : &str = "HOMECHATBOT_USERNAME";
const ENV_VAR_HOMECHATBOT_PASSWORD : &str = "HOMECHATBOT_PASSWORD";
const ENV_VAR_HOMECHATBOT_MONGO_ADDRESS : &str = "HOMECHATBOT_MONGO_ADDRESS";
const ENV_VAR_HOMECHATBOT_MONGO_USERNAME : &str = "HOMECHATBOT_MONGO_USERNAME";
const ENV_VAR_HOMECHATBOT_MONGO_PASSWORD : &str = "HOMECHATBOT_MONGO_PASSWORD";

async fn do_check_rooms(client: Box<Client>, db: Box<db::Homechatbotdb>) -> Result<()> {
    loop {
        let client_rooms = client.invited_rooms();
        if client_rooms.len() > 0 {
            println!("Number of rooms invited into: {}", client_rooms.len());
        }
        for cr in client_rooms {
            println!("Invited room details: {:?}", cr);
            let cm : &Common = &(*cr); // Deref trait to get inner of type Common
            let br : &BaseRoom = &(*cm); // Deref trait to get inner of type BaseRoom
            let cc = match br.create_content() {
                Some(cc) => cc.creator.into_string(),
                None => String::from("(none)"),
            };
            if db.is_valid_inviting_user(&cc).await {
                match cr.accept_invitation().await {
                    Ok(_) => {
                        println!("Room {} joined!", br.room_id())
                    },
                    Err(_) => {
                        println!("Unable to join room {}", br.room_id())
                    },
                };
            } else {
                println!("Rejecting invitation from {}", cc);
                match cr.reject_invitation().await {
                    Ok(_) => {
                        println!("Room {} rejected!", br.room_id())
                    },
                    Err(_) => {
                        println!("Unable to reject room {}", br.room_id())
                    },
                };
            }
        }
        thread::sleep(time::Duration::from_secs(1));
    }
}

async fn message_triage(msg: String, db: Box<db::Homechatbotdb>) -> String {
    if msg.to_lowercase().trim() == "test" {
        return String::from("running");
    } else if msg.to_lowercase().trim() == "help" {
        return String::from("The following commands are currently supported:
    bgchan
    gro / grocery");
    }
    let re = match Regex::new(r"^(?s)(\w+)\s+(.*)$") {
        Ok(r) => r,
        Err(e) => return String::from(format!("ERROR: {}", e)),
    };
    let caps = match re.captures(msg.as_str()) {
        Some(c) => c,
        None => return String::from("UNKNOWN"),
    };
    let cmd = match caps.get(1) {
        Some(c) => c.as_str().to_lowercase(),
        None => return String::from("UNKNOWN"),
    };
    println!("Got command: {}", cmd);
    if cmd == "bgchan" {
        let rest_command = match caps.get(2) {
            Some(c) => c.as_str(),
            None => return String::from("UNKNOWN"),
        };
        return bgchan::handle_bgchan_command(rest_command.to_string()).await;
    } else if cmd == "gro" || cmd == "grocery" {
        let rest_command = match caps.get(2) {
            Some(c) => c.as_str(),
            None => return String::from("UNKNOWN"),
        };
        return grocery::handle_grocery_command(rest_command.to_string(), db).await;
    }
    return String::from("UNKNOWN");
}

async fn handle_message<'a>(ev: SyncMessageEvent<MessageEventContent>, room: Room, client: Client, db: Box<db::Homechatbotdb>) {
    if let Some(my_user_id) = client.user_id().await {
        println!("sender check: {:?} {:?}", ev.sender, my_user_id);
        if ev.sender != my_user_id {
            if let MessageType::Text(cnt) = ev.content.msgtype {
                let cm : &Common = &(*room); // Deref trait to get inner of type Common
                let br : &BaseRoom = &(*cm); // Deref trait to get inner of type BaseRoom
                println!("Received a message {:?}, {:?}", cnt.body, br.room_id());
                let txt_msg = AnyMessageEventContent::RoomMessage(
                    MessageEventContent::text_plain(message_triage(cnt.body.trim().to_string(), db).await)
                );
                let txn_id = Uuid::new_v4();
                match client.room_send(br.room_id(), txt_msg, Some(txn_id)).await {
                    Ok(r) => {
                        println!("Response successfully sent: {:?}", r)
                    },
                    Err(e) => {
                        eprintln!("Unable to send response: {:?}", e)
                    },
                };
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let matrixusername = match env::var(ENV_VAR_HOMECHATBOT_USERNAME) {
        Ok(val) => val,
        Err(e) => {
            eprintln!("Unable to get value for environment variable {}: {}", ENV_VAR_HOMECHATBOT_USERNAME, e);
            process::exit(-1);
        },
    };
    if matrixusername == "" {
        eprintln!("Please set the environment variable {}", ENV_VAR_HOMECHATBOT_USERNAME);
        process::exit(-1);
    }
    let matrixpassword = match env::var(ENV_VAR_HOMECHATBOT_PASSWORD) {
        Ok(val) => val,
        Err(e) => {
            eprintln!("Unable to get value for environment variable {}: {}", ENV_VAR_HOMECHATBOT_PASSWORD, e);
            process::exit(-1);
        },
    };
    if matrixpassword == "" {
        eprintln!("Please set the environment variable {}", ENV_VAR_HOMECHATBOT_PASSWORD);
        process::exit(-1);
    }
    let mongodbaddress = match env::var(ENV_VAR_HOMECHATBOT_MONGO_ADDRESS) {
        Ok(val) => val,
        Err(e) => {
            eprintln!("Unable to get value for environment variable {}: {}", ENV_VAR_HOMECHATBOT_MONGO_ADDRESS, e);
            process::exit(-1);
        },
    };
    if mongodbaddress == "" {
        eprintln!("Please set the environment variable {}", ENV_VAR_HOMECHATBOT_MONGO_ADDRESS);
        process::exit(-1);
    }
    let mongodbuname = match env::var(ENV_VAR_HOMECHATBOT_MONGO_USERNAME) {
        Ok(val) => val,
        Err(e) => {
            eprintln!("Unable to get value for environment variable {}: {}", ENV_VAR_HOMECHATBOT_MONGO_USERNAME, e);
            process::exit(-1);
        },
    };
    if mongodbuname == "" {
        eprintln!("Please set the environment variable {}", ENV_VAR_HOMECHATBOT_MONGO_USERNAME);
        process::exit(-1);
    }
    let mongodbpass = match env::var(ENV_VAR_HOMECHATBOT_MONGO_PASSWORD) {
        Ok(val) => val,
        Err(e) => {
            eprintln!("Unable to get value for environment variable {}: {}", ENV_VAR_HOMECHATBOT_MONGO_PASSWORD, e);
            process::exit(-1);
        },
    };
    if mongodbpass == "" {
        eprintln!("Please set the environment variable {}", ENV_VAR_HOMECHATBOT_MONGO_PASSWORD);
        process::exit(-1);
    }
    println!("Env var check passed");

    let user = match UserId::try_from(matrixusername) {
        Ok(us) => us,
        Err(e) => {
            eprintln!("Unable to create a matrix user object: {}", e);
            process::exit(-1);
        },
    };
    let client = match Client::new_from_user_id(user.clone()).await {
        Ok(cl) => Box::new(cl),
        Err(e) => {
            eprintln!("Unable to create a matrix client object: {}", e);
            process::exit(-1);
        },
    };
    println!("User and client created");

    // First we need to log in.
    match client.login(user.localpart(), matrixpassword.as_str(), None, None).await {
        Ok(_) => (),
        Err(e) => {
            eprintln!("Unable to login: {}", e);
            process::exit(-1);
        },
    };
    println!("Successful login");

    let db = Box::new(match db::Homechatbotdb::new(mongodbaddress, mongodbuname, mongodbpass).await {
        Ok(d) => d,
        Err(e) => {
            eprintln!("DB error: {}", e);
            process::exit(-1);
        },
    });
    println!("DB connection successful");

    client.register_event_handler({
            let dbd = db.clone();
            move |ev: SyncMessageEvent<MessageEventContent>, room: Room, client: Client| {
                let dbd = dbd.clone();
                async move {
                    handle_message(ev, room, client, dbd).await;
                }
            }
        }
    ).await;
    println!("Event registered");

    tokio::spawn(do_check_rooms(client.clone(), db.clone()));
    println!("Room checker is running");

    // Syncing is important to synchronize the client state with the server.
    // This method will never return.
    client.clone().sync(SyncSettings::default()).await;

    Ok(())
}
