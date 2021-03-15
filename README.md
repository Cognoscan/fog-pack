# fog-pack

[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](
https://github.com/Cognoscan/fog-pack)
[![Cargo](https://img.shields.io/crates/v/fog-pack.svg)](
https://crates.io/crates/fog-pack)
[![Documentation](https://docs.rs/fog-pack/badge.svg)](
https://docs.rs/fog-pack)


## Examples

First, include fog-pack in your Cargo.toml: 

```toml
[dependencies]
fog-pack = "0.1.0"
```

Before anything else, we must initialize the underlying crypto library:

```
# use fog_pack::*;
crypto::init();
```

Generally, a schema is the first thing you'll want to make. This specifies the 
format of all our immutable documents, along with the entries attached to them:

```rust
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
```

With a schema in place, we can create documents that adhere to them, and entries 
to attach to those documents:

```rust
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
```

Entries are encoded fog-pack with an associated document and string field. They 
let us attach changing data to an immutable document, including links between 
documents.

Both documents and entries can be crytographically signed. This requires having 
a key vault in place, along with a key:

```rust
// Create a Vault for storing our Identity,
// which we'll use to sign posts.
let mut vault = crypto::Vault::new_from_password(
    crypto::PasswordLevel::Interactive,
    "Not a good password".to_string()
).unwrap();
let my_key = vault.new_key();

my_posts.sign(&vault, &my_key).unwrap();
first_post.sign(&vault, &my_key).unwrap();
```

Both documents and entries go through a schema to be encoded; this 
lets them be validated and optionally compressed:

```rust
let encoded_my_posts = schema.encode_doc(my_posts).unwrap();
let first_post_checklist = schema.encode_entry(first_post).unwrap();
let encoded_first_post = first_post_checklist.complete().unwrap();
```

Entries may require additional validation with documents they link to, but in 
this case, we don't need to do any additional validation and can retrieve the 
encoded entry right away.

Finally, where the schema allows it, we can make queries that will match against 
these entries:

```rust
// We can create a query to use for picking posts within a time window
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
```

## License

Licensed under either of

- Apache License, Version 2.0
	([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license
	([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
