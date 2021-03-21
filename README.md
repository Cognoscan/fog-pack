# fog-pack

[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](
https://github.com/Cognoscan/fog-pack)
[![Cargo](https://img.shields.io/crates/v/fog-pack.svg)](
https://crates.io/crates/fog-pack)
[![Documentation](https://docs.rs/fog-pack/badge.svg)](
https://docs.rs/fog-pack)

A serialization library for content-addressed, decentralized storage.

The fog-pack serialization format is designed from the ground-up to be effective and useful for
content-addressed storage systems, and to work effectively in a decentralized network. With
these being the highest priorities for the format, it has had to make some tough choices that
other serialization formats do not. Here's the quick rundown:

- It's a self-describing binary serialization format
- It builds on [`serde`](https://serde.rs/) for serialization of Rust structs
- It has a canonical form for all data. The same data will only ever have one valid serialized
    version of itself.
- It supports schema for verifying serialized data
- Schema may be serialized
- Data can be encapsulated into Documents, which can be tagged with a schema the data conforms
    to. Documents always have a cryptographic hash that uniquely identifies the data.
- Data can also be encapsulated into Entries, which are always associated with a parent
    document, and have a string for grouping them with other similar Entries.
- Documents and Entries may be **cryptographically signed**, which changes their identifying
    hashes.
- Documents and Entries may be **compressed with zstandard**, which does not change their
    identifying hashes. Zstandard dictionaries are supported when a schema is used.
- Documents and Entries are size-limited and have a limited nesting depth by design.
- Encrypted objects are available, using the
    [`fog-crypto`](https://crates.io/crates/fog-crypto) library.

# Key Concepts

- `Schemas`: A schema, which validates Documents and associated Entries, and can
    compress both of them
- `Documents`: A hashed piece of serialized data, which may adhere to a schema and
    be cryptographically signed.
- `Entries`: A hashed piece of serialized data, which has an associated parent
    document and key string. It may also be cryptographically signed.
- `Queries`: A query, which may be used to find entries attached to a Document.

These four types form the core of fog-pack's concepts, and are used to build up complex,
inter-related data in content-addressed storage systems.

So, what does it look like in use? Let's start with a simple idea: we want to make a streaming
series of small text posts. It's some kind of blog, so let's have there be an author, blog
title, and optional website link. Posts can be attached to the blog as entries, which will have
a creation timestamp, an optional title, and the post content.

We'll start by declaring the documents and the schema:

```rust
// Our Blog's main document
#[derive(Serialize, Deserialize)]
struct Blog {
    title: String,
    author: String,
    // We prefer to omit the field if it's set to None, which is not serde's default
    #[serde(skip_serializing_if = "Option::is_none")]
    link: Option<String>,
}

// Each post in our blog
#[derive(Serialize, Deserialize)]
struct Post {
    created: Timestamp,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
}

// Build our schema into a completed schema document.
let schema_doc = SchemaBuilder::new(MapValidator::new()
        .req_add("title", StrValidator::new().build())
        .req_add("author", StrValidator::new().build())
        .opt_add("link", StrValidator::new().build())
        .build()
    )
    .entry_add("post", MapValidator::new()
        .req_add("created", TimeValidator::new().query(true).ord(true).build())
        .opt_add("title", StrValidator::new().query(true).regex(true).build())
        .req_add("content", StrValidator::new().build())
        .build(),
        None
    )
    .build()
    .unwrap();
// For actual use, we'll turn the schema document into a Schema
let schema = Schema::from_doc(&schema_doc)?;
```

Now that we have our schema and structs, we can make a new blog and make posts to it. We'll
sign everything with a cryptographic key, so people can know we're the ones making these posts.
We can even make a query that can be used to search for specific posts!

```rust
// Brand new blog time!
let my_key = fog_crypto::identity::IdentityKey::new_temp(&mut rand::rngs::OsRng);
let my_blog = Blog {
    title: "Rusted Gears: A programming blog".into(),
    author: "ElectricCogs".into(),
    link: Some("https://cognoscan.github.io/".into()),
};
let my_blog = NewDocument::new(my_blog, Some(schema.hash()))?.sign(&my_key)?;
let my_blog = schema.validate_new_doc(my_blog)?;
let blog_hash = my_blog.hash();

// First post!
let new_post = Post {
    created: Timestamp::now().unwrap(),
    title: Some("My first post".into()),
    content: "I'm making my first post using fog-pack!".into(),
};
let new_post = NewEntry::new(new_post, "post", &blog_hash)?.sign(&my_key)?;

// We can find entries using a Query:
let query = NewQuery::new("post", MapValidator::new()
    .req_add("title", StrValidator::new().in_add("My first post").build())
    .build()
);

// To complete serialization of all these structs, we need to pass them through the schema one
// more time:
let (blog_hash, encoded_blog): (Hash, Vec<u8>) =
    schema.encode_doc(my_blog)?;
let (post_hash, encoded_post): (Hash, Vec<u8>) =
    schema.encode_new_entry(new_post)?.complete()?;
let encoded_query =
    schema.encode_query(query)?;

// Decoding is also done via the schema:
let my_blog = schema.decode_doc(encoded_blog)?;
let new_post = schema.decode_entry(encoded_post, "post", &blog_hash)?;
let query = schema.decode_query(encoded_query)?;
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
