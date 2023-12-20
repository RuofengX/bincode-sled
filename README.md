# bincode-sled - a database build on top of sled

This project is inspired by typed-sled, which is an awesome typed middleware
for sled.

## Difference between typed-sled

* Using bincode-v2 instead of seder to processor data.
* Delete the custom implement for data.

[![API](https://docs.rs/bincode-sled/badge.svg)](https://docs.rs/typed-sled)

sled is a high-performance embedded database with an API that is similar to a `BTreeMap<[u8], [u8]>`.  
bincode-sled builds on top of sled and offers an API that is similar to a `BTreeMap<K, V>`, where K and V are user defined types.

## Example

```rust
use bincode::{Encode, Decode};

#[derive(Debug, Clone, Encode, Decode, PartialEq)]
struct SomeValue(u32);

// Creating a temporary sled database
let db = sled::Config::new().temporary(true).open().unwrap();

// The id is used by sled to identify which Tree in the database (db) to open
let tree = bincode_sled::Tree::<String, SomeValue>::open(&db, "unique_id");

// insert and get, similar to std's BTreeMap
tree.insert(&"some_key".to_owned(), &SomeValue(10))?;

assert_eq!(tree.get(&"some_key".to_owned())?, Some(SomeValue(10)));
Ok(())


```

## features

Multiple features for common use cases are available:

- Search engine for searching through a tree's keys and values by using [tantivy].
- Automatic key generation.
- Converting one typed Tree to another typed Tree with different key and value types.

[sled]: https://github.com/spacejam/sled
[bincode]: https://github.com/bincode-org/bincode
[tantivy]: https://github.com/quickwit-inc/tantivy
