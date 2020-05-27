extern crate fog_pack;

use fog_pack::*;
use std::time::SystemTime;

fn main() {
    // Create a simple schema for streaming text posts
    let schema_doc = Document::new(fogpack!({
        "req": {
            "name": { "type": "Str" },
        },
        "entries": {
            "post": {
                "type": "Obj",
                "req": {
                    "text": { "type": "Str" },
                    "time": { "type": "Time", "query": true, "ord": true}
                }
            }
        }
    })).unwrap();
    let mut schema = Schema::from_doc(schema_doc).unwrap();

    // Create a new text post document
    let mut my_posts = Document::new(fogpack!({
        "": Value::from(schema.hash().clone()),
        "name": "My Text Posts",
    })).unwrap();

    // Make our first post
    let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap();
    let mut first_post = Entry::new(
        my_posts.hash().clone(), 
        "post".to_string(),
        fogpack!({
            "text": "This is my very first post.",
            "time": Timestamp::from_sec(now.as_secs() as i64)
        })
    ).unwrap();

    // Create a Vault for storing our Identity,
    // which we'll use to sign posts.
    let mut vault = crypto::Vault::new_from_password(
        crypto::PasswordLevel::Interactive,
        "Not a good password".to_string()
    ).unwrap();
    let my_key = vault.new_key();

    my_posts.sign(&vault, &my_key).unwrap();
    first_post.sign(&vault, &my_key).unwrap();

    // Now, run them through the schema and encode them
    let encoded_my_posts = schema.encode_doc(my_posts).unwrap();
    let first_post_checklist = schema.encode_entry(first_post).unwrap();
    let encoded_first_post = first_post_checklist.complete().unwrap();

    // Finally, we can create a query to use for picking posts within a time window
    let my_posts_hash = extract_schema_hash(&encoded_my_posts[..]).unwrap().unwrap();
    let query_last_day = Entry::new(
        my_posts_hash,
        "post".to_string(),
        fogpack!({
            "type": "Obj",
            "unknown_ok": true,
            "time": {
                "type": "Time",
                "min": (now.as_secs() as i64) - (24*60*60),
                "max": now.as_secs() as i64,
            }
        })
    ).unwrap();
    let query_last_day = encode_query(query_last_day);

}
