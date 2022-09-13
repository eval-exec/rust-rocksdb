extern crate ckb_rocksdb as rocksdb;

use crate::rocksdb::{
    prelude::*, MergeOperands, OptimisticTransaction, OptimisticTransactionDB,
    OptimisticTransactionOptions, Options, TemporaryDBPath, WriteBatch, WriteOptions,
};
use std::sync::Arc;
use std::thread;

#[test]
fn test_optimistic_transactiondb() {
    let n = TemporaryDBPath::new();
    {
        let db = OptimisticTransactionDB::open_default(&n).unwrap();
        db.put(b"k1", b"v1").unwrap();
        assert_eq!(db.get(b"k1").unwrap().unwrap().as_ref(), b"v1");
        assert_eq!(db.get_pinned(b"k1").unwrap().unwrap().as_ref(), b"v1");
    }
}

#[test]
fn write_batch_works() {
    let path = TemporaryDBPath::new();
    {
        let db = OptimisticTransactionDB::open_default(&path).unwrap();
        {
            // test put
            let mut batch = WriteBatch::default();
            assert!(db.get(b"k1").unwrap().is_none());
            assert_eq!(batch.len(), 0);
            assert!(batch.is_empty());
            let _ = batch.put(b"k1", b"v1111");
            assert_eq!(batch.len(), 1);
            assert!(!batch.is_empty());
            assert!(db.get(b"k1").unwrap().is_none());
            assert!(db.write(&batch).is_ok());
            let r: Result<Option<DBVector>, Error> = db.get(b"k1");
            assert!(r.unwrap().unwrap().to_utf8().unwrap() == "v1111");
        }
        {
            // test delete
            let mut batch = WriteBatch::default();
            let _ = batch.delete(b"k1");
            assert_eq!(batch.len(), 1);
            assert!(!batch.is_empty());
            assert!(db.write(&batch).is_ok());
            assert!(db.get(b"k1").unwrap().is_none());
        }
        {
            // test size_in_bytes
            let mut batch = WriteBatch::default();
            let before = batch.size_in_bytes();
            let _ = batch.put(b"k1", b"v1234567890");
            let after = batch.size_in_bytes();
            assert!(before + 10 <= after);
        }
    }
}

#[test]
pub fn test_optimistic_transaction() {
    let n = TemporaryDBPath::new();
    {
        let db = OptimisticTransactionDB::open_default(&n).unwrap();

        let trans = db.transaction_default();

        trans.put(b"k1", b"v1").unwrap();
        trans.put(b"k2", b"v2").unwrap();
        trans.put(b"k3", b"v3").unwrap();
        trans.put(b"k4", b"v4").unwrap();

        let trans_result = trans.commit();

        assert!(trans_result.is_ok());

        let trans2 = db.transaction_default();

        let mut iter = trans2.raw_iterator();

        iter.seek_to_first();

        assert!(iter.valid());
        assert_eq!(iter.key(), Some(b"k1".as_ref()));
        assert_eq!(iter.value(), Some(b"v1".as_ref()));

        iter.next();

        assert!(iter.valid());
        assert_eq!(iter.key(), Some(b"k2".as_ref()));
        assert_eq!(iter.value(), Some(b"v2".as_ref()));

        iter.next(); // k3
        iter.next(); // k4
        iter.next(); // invalid!

        assert!(!iter.valid());
        assert_eq!(iter.key(), None);
        assert_eq!(iter.value(), None);

        let trans3 = db.transaction_default();

        trans2.put(b"k2", b"v5").unwrap();
        trans3.put(b"k2", b"v6").unwrap();

        trans3.commit().unwrap();

        trans2.commit().unwrap_err();
    }
}

#[test]
pub fn test_optimistic_transaction_rollback_savepoint() {
    let path = TemporaryDBPath::new();
    {
        let mut opts = Options::default();
        opts.create_if_missing(true);

        let db = OptimisticTransactionDB::open(&opts, &path).unwrap();
        let write_options = WriteOptions::default();
        let optimistic_transaction_options = OptimisticTransactionOptions::new();

        let trans1 = db.transaction(&write_options, &optimistic_transaction_options);
        let trans2 = db.transaction(&write_options, &optimistic_transaction_options);

        trans1.put(b"k1", b"v1").unwrap();

        let k1_2 = trans2.get(b"k1").unwrap();
        assert!(k1_2.is_none());

        trans1.commit().unwrap();

        let k1_2 = trans2.get(b"k1").unwrap().unwrap();
        assert_eq!(&*k1_2, b"v1");

        trans1.delete(b"k1").unwrap();

        let k1_2 = trans2.get(b"k1").unwrap().unwrap();
        assert_eq!(&*k1_2, b"v1");

        trans1.rollback().unwrap();

        let k1_2 = trans2.get(b"k1").unwrap().unwrap();
        assert_eq!(&*k1_2, b"v1");

        trans1.delete(b"k1").unwrap();
        trans1.set_savepoint();
        trans1.put(b"k2", b"v2").unwrap();
        trans1.rollback_to_savepoint().unwrap();
        trans1.commit().unwrap();

        let k1_2 = trans2.get(b"k1").unwrap();
        assert!(k1_2.is_none());

        let k2_2 = trans2.get(b"k2").unwrap();
        assert!(k2_2.is_none());

        trans2.commit().unwrap();
    }
}

#[test]
pub fn test_optimistic_transaction_cf() {
    let path = TemporaryDBPath::new();
    {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        let mut db = OptimisticTransactionDB::open_cf(&opts, &path, &["cf1"]).unwrap();
        {
            let cf_handle = db.cf_handle("cf1").unwrap();
            let write_options = WriteOptions::default();
            let optimistic_transaction_options = OptimisticTransactionOptions::new();

            let trans = db.transaction(&write_options, &optimistic_transaction_options);

            trans.put_cf(cf_handle, b"k1", b"v1").unwrap();
            trans.commit().unwrap();

            let k1 = trans.get_cf(cf_handle, b"k1").unwrap().unwrap();
            assert_eq!(&*k1, b"v1");

            let k1 = trans.get_pinned_cf(cf_handle, b"k1").unwrap().unwrap();
            assert_eq!(&*k1, b"v1");

            trans.delete_cf(cf_handle, b"k1").unwrap();
            trans.commit().unwrap();
        }

        db.drop_cf("cf1").unwrap();
    }
}

#[test]
pub fn test_optimistic_transaction_snapshot() {
    let path = TemporaryDBPath::new();
    {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        let db = OptimisticTransactionDB::open(&opts, &path).unwrap();

        let write_options = WriteOptions::default();
        let optimistic_transaction_options = OptimisticTransactionOptions::new();
        let trans1 = db.transaction(&write_options, &optimistic_transaction_options);

        let mut optimistic_transaction_options_snapshot = OptimisticTransactionOptions::new();
        optimistic_transaction_options_snapshot.set_snapshot(true);
        // create transaction with snapshot
        let trans2 = db.transaction(&write_options, &optimistic_transaction_options_snapshot);

        trans1.put(b"k1", b"v1").unwrap();

        let k1_2 = trans2.get(b"k1").unwrap();
        assert!(k1_2.is_none());

        {
            let k1_3 = trans2.get_pinned(b"k1").unwrap();
            assert!(k1_3.is_none());
        }

        trans1.commit().unwrap();

        trans2.commit().unwrap();
        drop(trans2);

        let trans3 = db.transaction(&write_options, &optimistic_transaction_options_snapshot);

        trans1.delete(b"k1").unwrap();
        trans1.commit().unwrap();

        assert!(trans3.get(b"k1").unwrap().is_none());

        {
            let snapshot = trans3.snapshot();
            let k1_3 = snapshot.get(b"k1").unwrap().unwrap();
            assert_eq!(&*k1_3, b"v1");

            let k1_4 = snapshot.get_pinned(b"k1").unwrap().unwrap();
            assert_eq!(&*k1_4, b"v1");
        }

        trans3.commit().unwrap();
        drop(trans3);

        let trans4 = db.transaction(&write_options, &optimistic_transaction_options_snapshot);

        let k1_4 = trans4.snapshot().get(b"k1").unwrap();
        assert!(k1_4.is_none());

        trans4.commit().unwrap();
    }
}

#[test]
pub fn test_optimistic_transaction_merge() {
    #[allow(clippy::unnecessary_wraps)]
    fn concat_merge(
        _new_key: &[u8],
        existing_val: Option<&[u8]>,
        operands: &mut MergeOperands,
    ) -> Option<Vec<u8>> {
        let mut result: Vec<u8> = Vec::with_capacity(operands.size_hint().0);
        if let Some(v) = existing_val {
            for e in v {
                result.push(*e)
            }
        }
        for op in operands {
            for e in op {
                result.push(*e)
            }
        }
        Some(result)
    }

    let path = TemporaryDBPath::new();

    {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.set_merge_operator_associative("test operator", concat_merge);
        let db = OptimisticTransactionDB::open(&opts, &path).unwrap();
        let trans = db.transaction_default();

        trans.put(b"k1", b"a").unwrap();
        trans.merge(b"k1", b"b").unwrap();
        trans.merge(b"k1", b"c").unwrap();
        trans.merge(b"k1", b"d").unwrap();
        trans.merge(b"k1", b"efg").unwrap();
        // https://github.com/facebook/rocksdb/blob/v6.28.2/HISTORY.md#6200-2021-04-16
        // since 6.20.0, .transaction return the correct merged result.
        assert_eq!(&*trans.get(b"k1").unwrap().unwrap(), b"abcdefg");
        trans.commit().unwrap();

        let k1 = trans.get(b"k1").unwrap().unwrap();
        assert_eq!(&*k1, b"abcdefg");

        trans.commit().unwrap();
    }
}

#[derive(Clone)]
struct TransWrapper {
    txn: Arc<OptimisticTransaction>,
}

impl TransWrapper {
    fn new(txn: OptimisticTransaction) -> Self {
        Self { txn: Arc::new(txn) }
    }

    fn check<K>(&self, key: K, value: &str) -> bool
    where
        K: AsRef<[u8]>,
    {
        self.txn.get(key).unwrap().unwrap().to_utf8().unwrap() == value
    }
}

#[test]
fn sync_transaction_test() {
    let n = TemporaryDBPath::new();
    {
        let db = OptimisticTransactionDB::open_default(&n).unwrap();
        let txn = db.transaction_default();

        assert!(txn.put(b"k1", b"v1").is_ok());
        assert!(txn.put(b"k2", b"v2").is_ok());

        let wrapper = TransWrapper::new(txn);

        let wrapper_1 = wrapper.clone();
        let handler_1 = thread::spawn(move || wrapper_1.check("k1", "v1"));

        let wrapper_2 = wrapper;
        let handler_2 = thread::spawn(move || wrapper_2.check("k2", "v2"));

        assert!(handler_1.join().unwrap());
        assert!(handler_2.join().unwrap());
    }
}
