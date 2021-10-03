use crate::db;
use regex::Regex;
use serde::{Deserialize, Serialize};
use mongodb::bson::doc;

const GROCERY_COLLECTION_NAME : &str = "groceries";
const GROCERY_HELP : &str = "Grocery allowed commands:
    list [category]
    add {category}
        product1
        product2
        product3
        ...
    rem {product_id}";
const MAX_ITEMS_IN_DB : u32 = 10000;

#[derive(Debug, Serialize, Deserialize)]
struct Groceries {
    category: String,
    groid: u32,
    product: String,
}

pub async fn handle_grocery_command(cmd: String, db: Box<db::Homechatbotdb>) -> String {
    match db.check_collection_exists(GROCERY_COLLECTION_NAME).await {
        Ok(exists) => {
            if !exists {
                match db.create_collection(GROCERY_COLLECTION_NAME).await {
                    Ok(_) => {},
                    Err(e) => return String::from(format!("{}", e)),
                };
            }
        },
        Err(e) => return String::from(format!("{}", e)),
    };
    match db.get_collection_index(GROCERY_COLLECTION_NAME).await {
        Ok(indxs) => {
            let mut groid_inside = false;
            for indx in indxs {
                if indx.starts_with("groid") {
                    groid_inside = true;
                    break;
                }
            }
            if !groid_inside {
                println!("Setting grocery ID as index");
                match db.create_collection_index(GROCERY_COLLECTION_NAME, "groid").await {
                    Ok(_) => {},
                    Err(e) => return String::from(format!("{}", e)),
                };
            }
        },
        Err(e) => return String::from(format!("{}", e)),
    };
    let re = match Regex::new(r"^(?s)(\w+)(?:\s+(.*))?$") {
        Ok(r) => r,
        Err(e) => return String::from(format!("ERROR: {}", e)),
    };
    let caps = match re.captures(cmd.as_str()) {
        Some(c) => c,
        None => return String::from(GROCERY_HELP),
    };
    let cmd = match caps.get(1) {
        Some(c) => c.as_str().to_lowercase(),
        None => return String::from(GROCERY_HELP),
    };
    if cmd == "list" {
        let fltr = match caps.get(2) {
            Some(c) => vec![c.as_str()],
            None => vec![],
        };
        return handle_list_request(fltr, db).await;
    } else if cmd == "add" {
        match caps.get(2) {
            Some(c) => return handle_add_request(c.as_str(), db).await,
            None => return String::from(GROCERY_HELP),
        };
    } else if cmd == "rem" {
        match caps.get(2) {
            Some(c) => return handle_remove_request(c.as_str(), db).await,
            None => return String::from(GROCERY_HELP),
        };
    }
    return String::from(GROCERY_HELP);
}

async fn handle_add_request(cmd_rest: &str, db: Box<db::Homechatbotdb>) -> String {
    let re = match Regex::new(r"^(?s)(.*?)\n(.*)$") {
        Ok(r) => r,
        Err(e) => return String::from(format!("ERROR: {}", e)),
    };
    let caps = match re.captures(cmd_rest) {
        Some(c) => c,
        None => return String::from(GROCERY_HELP),
    };
    let category = match caps.get(1) {
        Some(c) => c.as_str(),
        None => return String::from(GROCERY_HELP),
    };
    let products = match caps.get(2) {
        Some(p) => p.as_str(),
        None => return String::from(GROCERY_HELP),
    };
    let prodarr = products.split("\n");
    for sprod in prodarr {
        if sprod.trim() == "" {
            continue;
        }
        let mut success = false;
        while !success {
            let id = match get_smallest_available_id(db.clone()).await {
                Ok(i) => i,
                Err(e) => return String::from(format!("ERROR: {}", e)),
            };
            match db.insert_data_to_collection(GROCERY_COLLECTION_NAME, vec![doc! {"product": sprod, "category": category, "groid": id}]).await {
                Ok(_) => {
                    success = true;
                },
                Err(e) => {
                    let err = format!("{}", e);
                    if err.contains("E11000 duplicate key error collection") {
                        continue;
                    } else {
                        return err;
                    }
                },
            };
        }
    }
    return String::from("Items successfully added!");
}

async fn get_smallest_available_id(db: Box<db::Homechatbotdb>) -> Result<u32, String> {
    let items = match db.get_generic_data_collection::<Groceries>(GROCERY_COLLECTION_NAME, doc!{}, doc!{}).await {
        Ok(i) => i,
        Err(e) => return Err(String::from(format!("{}", e))),
    };
    let mut bufv : Vec<u32> = vec![];
    for pro in items {
        bufv.push(pro.groid);
    }
    for n in 1..MAX_ITEMS_IN_DB {
        if !bufv.contains(&n) {
            return Ok(n);
        }
    }
    return Err(format!("Too many products in the database"))
}

async fn handle_list_request(spec_cat: Vec<&str>, db: Box<db::Homechatbotdb>) -> String {
    let fltr = if spec_cat.len() > 0 {
        doc!{"category": spec_cat[0]}
    } else {
        doc!{}
    };
    let items = match db.get_generic_data_collection::<Groceries>(GROCERY_COLLECTION_NAME, fltr, doc!{"category":1}).await {
        Ok(i) => i,
        Err(e) => return format!("Error getting groceries: {}", e).to_string(),
    };
    let mut msg : String = if items.len() > 0 {
        "".to_string()
    } else {
        "List is empty".to_string()
    };
    let mut prev_cat : String = "".to_string();
    for pro in items {
        if pro.category != prev_cat {
            msg = format!("{}{}:\n", msg, pro.category);
            prev_cat = pro.category;
        }
        msg = format!("{}({}) {}\n", msg, pro.groid, pro.product);
    }
    return msg;
}

async fn handle_remove_request(cmd_rest: &str, db: Box<db::Homechatbotdb>) -> String {
    let items = cmd_rest.split(",");
    for itm in items {
        let id = match itm.parse::<u32>() {
            Ok(i) => i,
            Err(e) => return format!("Only numbers are allowed: {}\n{}", e, GROCERY_HELP).to_string(),
        };
        match db.remove_data(GROCERY_COLLECTION_NAME, doc!{"groid":id}).await {
            Ok(_) => {},
            Err(e) => return format!("{}", e),
        };
    }
    return "Items successfully removed".to_string();
}