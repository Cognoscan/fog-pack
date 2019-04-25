extern crate condense_db;

use condense_db::*;

fn main() {
    println!("Start up the system");
    let db = Db::new();
    let mut vault = crypto::Vault::new_from_password(
        crypto::PasswordLevel::Interactive,
        String::from("BadPassword")).unwrap();

    println!("Generate a new ID");
    let my_key = vault.new_key(); 

    println!("Making a document");
    let mut test_doc = Document::new(msgpack!({
        "": Hash::new_empty(),
        "Title": "Test document",
        "Description": "This is a test document",
    })).unwrap();
    test_doc.sign(&vault, &my_key);
    let doc_permission = Permission::new();
    let doc_hash = test_doc.hash();

    println!("Adding a document");
    let res = db.add_doc(test_doc, doc_permission, 0).unwrap();
    let res = res.recv();
    println!("Got back: {:?}", res);

    println!("Making an entry");
    let test_entry = Entry::new_signed(doc_hash.clone(), String::from("field"), msgpack!({
        "Time": Timestamp::now().unwrap(),
        "Description": "First entry",
    }), &vault, &my_key).unwrap();

    println!("Adding an entry");
    let res = db.add_entry(test_entry, 0).unwrap();
    let res = res.recv();
    println!("Got back: {:?}", res);

    println!("Retrieving a document");

    println!("Deleting a document");
    let res = db.del_doc(doc_hash.clone()).unwrap();
    let res = res.recv();
    println!("Got back: {:?}", res);

    println!("Closing the database");
    db.close().unwrap();
}
