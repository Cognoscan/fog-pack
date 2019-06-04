extern crate condense_db;

use condense_db::*;

fn main() {
    println!("Start up the system");
    crypto::init().expect("Couldn't initialize random-number generator");
    let db = Db::new("simple_setup").unwrap();
    let mut vault = crypto::Vault::new_from_password(
        crypto::PasswordLevel::Interactive,
        String::from("BadPassword")).unwrap();

    println!("Generate a new ID");
    let my_key = vault.new_key(); 

    println!("Setting up a simple schema");
    let test_schema = Document::new(msgpack!({
        //"": Hash::new_empty(),
        "name": "Simple chat schema",
        "req": {
            "title": { "type": "Str", "max_len": 255 },
            "description": { "type": "Str" }
        },
        "entries": {
            "post" : {
                "type": "Obj",
                "req": {
                    "time": { "type": "Time" },
                    "text": { "type": "Str" }
                }
            }
        }
    })).unwrap();
    let schema_hash = test_schema.hash();
    let schema_permission = Permission::new().local_net(true).direct(true);
    let res = db.add_doc(test_schema, &schema_permission, 0).unwrap();
    let res = res.recv().unwrap();
    println!("    Got back: {:?}", res);

    println!("Making a document");
    let test_doc = Document::new(msgpack!({
        "": schema_hash.clone(),
        "title": "Test chat",
        "description": "This is a test chat",
    })).unwrap();
    let doc_permission = schema_permission.clone().advertise(true);
    let doc_hash = test_doc.hash();

    println!("Setting up query, then adding document");
    let mut query = Query::new();
    query.add_root(&doc_hash);
    let query_res = db.query(query, &doc_permission, 1).unwrap();
    let res = db.add_doc(test_doc, &doc_permission, 0).unwrap();
    let res = res.recv().unwrap();
    println!("    Got back: {:?}", res);

    println!("Trying to delete the schema while it is in use");
    let res = db.del_doc(schema_hash.clone()).unwrap();
    let res = res.recv().unwrap();
    println!("    Got back: {:?}", res);


    println!("Making an entry");
    let mut test_entry = Entry::new(doc_hash.clone(), String::from("post"), msgpack!({
        "time": Timestamp::now().unwrap(),
        "text": "Making a post",
    })).unwrap();
    test_entry.sign(&vault, &my_key).unwrap();
    let res = db.add_entry(test_entry, 0).unwrap();
    let res = res.recv().unwrap();
    println!("    Got back: {:?}", res);

    println!("Retrieving a document");
    loop {
        let query_result = query_res.recv().unwrap();
        match query_result {
            QueryResponse::Doc((doc, effort)) => {
                println!("    Got a document back, effort = {}", effort);
                println!("    Document is:");
                println!("{}", doc.get_value());
                println!("");
            },
            QueryResponse::Entry(_) => {
                println!("    Got an entry back");
            },
            QueryResponse::Invalid => {
                println!("    Invalid query");
                break;
            }
            QueryResponse::DoneForever => {
                println!("    Done forever");
                break;
            },
            QueryResponse::BadDoc(_) => {
                println!("    BadDoc: One of the root hashes refers to a document that fails schema checks");
            }
            QueryResponse::UnknownSchema(_) => {
                println!("    UnknownSchema: One of the root hashes mapped to a document that has an unrecognized schema");
            }
        };
    }
    drop(res); // Done with query response

    println!("Deleting a document");
    let res = db.del_doc(doc_hash.clone()).unwrap();
    let res = res.recv().unwrap();
    println!("    Got back: {:?}", res);

    println!("Deleting the schema");
    let res = db.del_doc(schema_hash.clone()).unwrap();
    let res = res.recv().unwrap();
    println!("    Got back: {:?}", res);

    println!("Closing the database");
    db.close().unwrap();
}
