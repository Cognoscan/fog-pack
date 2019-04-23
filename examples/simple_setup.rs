extern crate condense_db;

use condense_db::*;

fn main() {
    let db = Db::new();

    println!("Adding a document");
    let doc = Doc { content: vec![254,1,2] };
    let doc_permission = Permission { perm: Vec::new() };

    let res = db.make_change(ChangeRequest::AddDoc((doc.clone(), doc_permission.clone(), 0))).unwrap();
    let res = res.recv();
    println!("Got back: {:?}", res);

    let res = db.make_change(ChangeRequest::DelDoc(Hash { x: 0 })).unwrap();
    let res = res.recv();
    println!("Got back: {:?}", res);

    println!("Closing the database");
    db.close().unwrap();
}
