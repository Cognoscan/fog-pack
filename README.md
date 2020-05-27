# fog-pack

[Documentation](https://docs.rs/fog-pack) | [Specs](https://docs.rs/fog-pack/0.1.0/fog-pack/spec/index.html)

fog-pack builds on msgpack with a set of extensions useful for all structured 
data. The ultimate goal is to provide a single coherent approach to encoding 
data, validating it, and compressing it. Any existing data format can be 
replaced with fog-pack, without compromise.

To meet this lofty goal, it extends msg-pack by providing:

- A canonical form for all data. Given a known input, the same fog-pack value 
	will always be generated
- Cryptographic hashes are a value type, and the hash of a fog-pack value can 
	be calculated
- Encrypted data is a value type, which may contain arbitrary data, a secret 
	key, or a private key
- Support for symmetric-key cryptography
	- Data can be encrypted using a secret key
	- Secret keys may be passed around in encrypted form
- Support for public-key cryptography.
	- Public keys are a value type
	- Data can be signed with a secret key
	- Data can be encrypted with a public key
	- Private keys may be passed around in encrypted form
- A schema format, allowing for validation of fog-pack values
	- Specifies subsets of possible values
	- Schema may be used to filter fog-pack values, allowing them to be used as a 
		query against a database of values
	- Schema are themselves fog-pack objects
- Immutable Documents, consisting of a fog-pack object with an optional schema 
	reference.
- Entries, consisting of a fog-pack object, a key string, and the hash of a 
	fog-pack Document. These may be used to form mutable links between documents.
- Support for compression. A document or entry may be compressed after encoding 
	& hashing. Dictionary compression of values is supported if a schema is used, 
	allowing for exceptionally fast compression and high ratios. See 
	[`zstd`](https://facebook.github.io/zstd/) for more information on the 
	compression used.

## Examples

First, include fog-pack in your Cargo.toml: 

```toml
[dependencies]
fog-pack = "0.1.0"
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
