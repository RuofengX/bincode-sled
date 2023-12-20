//! Create [Tree]s with automatically generated keys.
//!
//! # Example
//! ```
//! use typed_sled::key_generating::CounterTree;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // If you want to persist the data use sled::open instead
//!     let db = sled::Config::new().temporary(true).open().unwrap();
//!
//!     // The id is used by sled to identify which Tree in the database (db) to open.
//!     let tree = CounterTree::open(&db, "unique_id");
//!
//!     let (first_key, _) = tree.insert(&5)?;
//!     let (second_key, _) = tree.insert(&6)?;
//!
//!     assert_eq!(first_key, 0);
//!     assert_eq!(second_key, 1);
//!     assert_eq!(tree.get(&0)?, Some(5));
//!     assert_eq!(tree.get(&1)?, Some(6));
//!     Ok(())
//! }
//! ```
use crate::{Batch, Key, Tree, Value};
use sled::transaction::{ConflictableTransactionResult, TransactionResult};
use sled::Result;
use std::ops::Deref;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Wraps a type that implements KeyGenerating and uses it to
/// generate the keys for a bincode_sled::Tree.
///
/// See CounterTree for a specific example of how to use this type.
#[derive(Clone, Debug)]
pub struct KeyGeneratingTree<KG: KeyGenerating<K, V>, K, V>
where
    K: Key,
    V: Value,
{
    key_generator: KG,
    inner: Tree<K, V>,
}

impl<KG: KeyGenerating<K, V>, K, V> KeyGeneratingTree<KG, K, V>
where
    KG: KeyGenerating<K, V>,
    K: Key,
    V: Value,
{
    pub fn open<T: AsRef<str>>(db: &sled::Db, id: T) -> Self {
        let tree = Tree::open(db, id);
        let key_generator = KG::initialize(&tree);

        Self {
            key_generator,
            inner: tree,
        }
    }

    /// Insert a generated key to a new value, returning the key and the last value if it was set.
    pub fn insert(&self, value: &V) -> Result<(K, Option<V>)> {
        let key = self.key_generator.next_key();
        let res = self.inner.insert(&key, value);
        res.map(|opt_v| (key, opt_v))
    }

    /// Insert a key to a new value, returning the last value if it was set.
    /// Be careful not to insert a key that conflicts with the keys generated
    /// by the key generator. If you need the generated key for construction of
    /// the value, you can first use `next_key` and then use this method with
    /// the generated key. Alternatively use `insert_fn`.
    pub fn insert_with_key(&self, key: &K, value: &V) -> Result<Option<V>> {
        self.inner.insert(key, value)
    }

    pub fn next_key(&self) -> K {
        self.key_generator.next_key()
    }

    /// Insert a generated key to a new dynamically created value, returning the key and the last value if it was set.
    /// The argument supplied to `f` is a reference to the key and the returned value is the value that will
    /// be inserted at the key.
    pub fn insert_fn(&self, f: impl Fn(&K) -> V) -> Result<(K, Option<V>)> {
        let key = self.key_generator.next_key();
        let value = f(&key);
        let res = self.insert_with_key(&key, &value);
        res.map(|opt_v| (key, opt_v))
    }

    pub fn transaction<F, A, E>(&self, f: F) -> TransactionResult<A, E>
    where
        F: Fn(&KeyGeneratingTransactionalTree<KG, K, V>) -> ConflictableTransactionResult<A, E>,
    {
        self.inner.transaction(|transactional_tree| {
            f(&KeyGeneratingTransactionalTree {
                key_generator: self.key_generator(),
                inner: transactional_tree,
            })
        })
    }

    pub fn key_generator(&self) -> &KG {
        &self.key_generator
    }

    pub fn new_batch(&self) -> KeyGeneratingBatch<KG, K, V> {
        KeyGeneratingBatch {
            key_generator: self.key_generator(),
            inner: Batch::default(),
        }
    }

    pub fn apply_batch(&self, batch: KeyGeneratingBatch<KG, K, V>) -> Result<()> {
        self.inner.apply_batch(batch.inner)
    }
}

impl<KG: KeyGenerating<K, V>, K, V> Deref for KeyGeneratingTree<KG, K, V>
where
    K: Key,
    V: Value,
{
    type Target = Tree<K, V>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// Implement on a type that you want to use for key generation
/// for a typed_sled::Tree.
///
/// See CounterTree for a specific example of how to use this trait.
pub trait KeyGenerating<K, V>
where
    K: Key,
    V: Value,
{
    fn initialize(tree: &Tree<K, V>) -> Self;

    fn next_key(&self) -> K;
}

#[derive(Clone, Debug)]
pub struct KeyGeneratingBatch<'a, KG: KeyGenerating<K, V>, K, V>
where
    KG: KeyGenerating<K, V>,
    K: Key,
    V: Value,
{
    key_generator: &'a KG,
    inner: Batch<K, V>,
}

impl<'a, KG: KeyGenerating<K, V>, K: Key, V: Value> KeyGeneratingBatch<'a, KG, K, V> {
    pub fn insert(&mut self, value: &V) {
        self.inner.insert(&self.key_generator.next_key(), value);
    }

    pub fn remove(&mut self, key: &K) {
        self.inner.remove(key)
    }
}

/// A typed_sled::Tree with automatically generated and continuously increasing u64 keys.
pub type CounterTree<K, V> = KeyGeneratingTree<Counter, K, V>;

#[derive(Debug, Clone)]
pub struct Counter(Arc<AtomicU64>);

impl<V: Value> KeyGenerating<u64, V> for Counter {
    fn initialize(tree: &Tree<u64, V>) -> Self {
        if let Some((key, _)) = tree
            .last()
            .expect("KeyGenerating Counter failed to access sled Tree.")
        {
            Counter(Arc::new(AtomicU64::new(key + 1)))
        } else {
            Counter(Arc::new(AtomicU64::new(0)))
        }
    }

    fn next_key(&self) -> u64 {
        self.0.fetch_add(1, Ordering::Relaxed)
    }
}

pub struct KeyGeneratingTransactionalTree<'a, KG: KeyGenerating<K, V>, K, V>
where
    KG: KeyGenerating<K, V>,
    K: Key,
    V: Value,
{
    key_generator: &'a KG,
    inner: &'a crate::transaction::TransactionalTree<'a, K, V>,
}

impl<'a, KG: KeyGenerating<K, V>, K, V> KeyGeneratingTransactionalTree<'a, KG, K, V>
where
    K: Key,
    V: Value,
{
    pub fn insert(
        &self,
        value: &V,
    ) -> std::result::Result<Option<V>, sled::transaction::UnabortableTransactionError> {
        self.inner.insert(&self.key_generator.next_key(), value)
    }

    pub fn apply_batch(
        &self,
        batch: &KeyGeneratingBatch<KG, K, V>,
    ) -> std::result::Result<(), sled::transaction::UnabortableTransactionError> {
        self.inner.apply_batch(&batch.inner)
    }
}

impl<'a, KG: KeyGenerating<K, V>, K, V> Deref for KeyGeneratingTransactionalTree<'a, KG, K, V>
where
    K: Key,
    V: Value,
{
    type Target = crate::transaction::TransactionalTree<'a, K, V>;

    fn deref(&self) -> &Self::Target {
        self.inner
    }
}
