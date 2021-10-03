use mongodb::{Client, options::ClientOptions, IndexModel, options::IndexOptions, options::FindOptions};
use mongodb::bson::{doc, Document};
use serde::{Deserialize, Serialize};
use futures::stream::{StreamExt, TryStreamExt};

const DB_NAME : &str = "homechatbot_db";
const CONFIG_COLLECTION_NAME : &str = "config";

#[derive(Clone)]
pub struct Homechatbotdb {
    client: Client,
}

#[derive(Debug, Serialize, Deserialize)]
struct AllowedUsers {
    allowed_users: Vec<String>,
}

impl Homechatbotdb {
    pub async fn new(address: String, username: String, password: String) -> Result<Homechatbotdb, String> {
        let client_options = match ClientOptions::parse(format!("mongodb://{}:{}@{}/", username, password, address)).await {
            Ok(co) => co,
            Err(e) => return Err(String::from(format!("Unable tp create client options: {}", e))),
        };
        let client = match Client::with_options(client_options) {
            Ok(c) => c,
            Err(e) => return Err(String::from(format!("Unable to create DB client: {}", e))),
        };
        let hmcb = Homechatbotdb{client: client};
        match hmcb.check_db_exists(DB_NAME).await {
            Ok(r) => {
                if !r {
                    return Err(String::from(format!("Homechatbot DB \"{}\" not found", DB_NAME)))
                }
            },
            Err(e) => return Err(String::from(format!("{}", e))),
        };
        match hmcb.check_collection_exists(CONFIG_COLLECTION_NAME).await {
            Ok(r) => {
                if !r {
                    return Err(String::from(format!("Homechatbot config collection \"{}\" not found", CONFIG_COLLECTION_NAME)))
                }
            },
            Err(e) => return Err(String::from(format!("{}", e))),
        };
        return Ok(hmcb);
    }

    async fn check_db_exists(&self, dbname: &str) -> Result<bool, String> {
        let dbs = &self.client.list_database_names(None, None).await;
        let dbs = match dbs {
            Ok(d) => d,
            Err(e) => return Err(String::from(format!("Unable to list databases: {}", e))),
        };
        let mut has_db = false;
        for db in dbs {
            if db == dbname {
                has_db = true;
                break;
            }
        }
        return Ok(has_db);
    }

    pub async fn check_collection_exists(&self, coll_name: &str) -> Result<bool, String> {
        let db = &self.client.database(DB_NAME);
        let colls = match db.list_collection_names(None).await {
            Ok(c) => c,
            Err(e) => return Err(String::from(format!("Unable to list collections: {}", e))),
        };
        let mut has_coll = false;
        for coll in colls {
            if coll == coll_name {
                has_coll = true;
                break;
            }
        }
        return Ok(has_coll);
    }

    pub async fn create_collection(&self, coll_name: &str) -> Result<(), String> {
        let db = &self.client.database(DB_NAME);
        match db.create_collection(coll_name, None).await {
            Ok(_) => return Ok(()),
            Err(e) => return Err(String::from(format!("Unable to create collection: {}", e))),
        };
    }

    pub async fn get_collection_index(&self, name: &str) -> Result<Vec<String>, String> {
        let db = &self.client.database(DB_NAME);
        let coll = db.collection::<Document>(name);
        match coll.list_index_names().await {
            Ok(i) => return Ok(i),
            Err(e) => return Err(format!("Cannot get indexes: {}", e).to_string()),
        };
    }

    pub async fn create_collection_index(&self, coll_name: &str, index_name: &str) -> Result<(), String> {
        let db = &self.client.database(DB_NAME);
        let coll = db.collection::<Document>(coll_name);
        let imo = IndexOptions::builder().unique(true).build();
        let im = IndexModel::builder().keys(doc!{index_name:1}).options(imo).build();
        match coll.create_index(im, None).await {
            Ok(_) => return Ok(()),
            Err(e) => return Err(format!("Cannot create index: {}", e).to_string()),
        };
    }

    pub async fn is_valid_inviting_user(&self, userid: &String) -> bool {
        let db = &self.client.database(DB_NAME);
        let typed_collection = db.collection::<AllowedUsers>(CONFIG_COLLECTION_NAME);
        let filter = doc! {"allowed_users": {"$exists": true}};
        let mut cursor = match typed_collection.find(filter, None).await {
            Ok(c) => c,
            Err(_) => return false,
        };
        loop {
            match cursor.next().await {
                Some(v) => {
                    let obj = match v {
                        Ok(o) => o,
                        Err(_) => continue,
                    };
                    for allu in obj.allowed_users {
                        println!("Comparing inviting user \"{}\" with user in DB: \"{}\"", *userid, allu);
                        if allu == *userid {
                            return true;
                        }
                    }
                },
                None => break,
            }
        }
        return false;
    }

    pub async fn get_generic_data_collection<T>(&self, coll_name: &str, filter: Document, sort: Document) -> Result<Vec<T>, String>
    where
    for<'de> T: Deserialize<'de> + Sync + Unpin + Send {
        let db = &self.client.database(DB_NAME);
        let coll = db.collection::<T>(coll_name);
        let filo = FindOptions::builder().sort(sort).build();
        let cursor = match coll.find(filter, filo).await {
            Ok(c) => c,
            Err(e) => return Err(format!("Unable to get cursor: {}", e).to_string()),
        };
        match cursor.try_collect().await {
            Ok(v) => return Ok(v),
            Err(e) => return Err(format!("Unable to retrieve items: {}", e).to_string()),
        }
    }

    pub async fn insert_data_to_collection(&self, coll_name: &str, docs: Vec<Document>) -> Result<(), String> {
        let db = &self.client.database(DB_NAME);
        let coll = db.collection::<Document>(coll_name);
        match coll.insert_many(docs, None).await {
            Ok(_) => return Ok(()),
            Err(e) => return Err(format!("Unable to insert items: {}", e)),
        };
    }

    pub async fn remove_data(&self, coll_name: &str, fltr: Document) -> Result<(), String> {
        let db = &self.client.database(DB_NAME);
        let coll = db.collection::<Document>(coll_name);
        match coll.delete_many(fltr, None).await {
            Ok(_) => return Ok(()),
            Err(e) => return Err(format!("Unable to remove items: {}", e)),
        };
    }
}