#![allow(non_upper_case_globals)]
use anyhow::{anyhow, Result};
use bincode::config;
pub use bincode::{Decode, Encode};
pub use moka::notification::RemovalCause;
use moka::{sync::Cache, Expiry};
#[allow(unused_imports)]
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    sync::Arc,
    sync::OnceLock,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Expiration {
    Never,
    Second(u64),
    Minute(u64),
    Hour(u64),
}

impl Expiration {
    pub fn as_duration(&self) -> Option<Duration> {
        match self {
            Expiration::Never => None,
            Expiration::Second(v) => Some(Duration::from_secs(*v)),
            Expiration::Minute(v) => Some(Duration::from_secs(*v * 60)),
            Expiration::Hour(v) => Some(Duration::from_secs(*v * 60 * 60)),
        }
    }
}

pub struct CacheExpiry;

pub type CacheData = (Expiration, Vec<u8>);

type AppCache = Cache<String, (Expiration, Vec<u8>)>;

impl Expiry<String, (Expiration, Vec<u8>)> for CacheExpiry {
    #[allow(unused_variables)]
    fn expire_after_create(
        &self,
        key: &String,
        value: &(Expiration, Vec<u8>),
        current_time: Instant,
    ) -> Option<Duration> {
        value.0.as_duration()
    }
}

static CacheHand: OnceLock<AppCache> = OnceLock::new();

//初始化缓存
pub fn setup(
    callback: Option<fn(Arc<String>, CacheData, RemovalCause)>,
    max_cap: u64,
) -> Result<()> {
    let mut c = Cache::builder()
        .max_capacity(max_cap)
        .expire_after(CacheExpiry {});

    if let Some(callback) = callback {
        c = c.eviction_listener(callback);
    }
    let c = c.build();
    CacheHand
        .set(c)
        .map_err(|e| anyhow!("setup cache error:{:?}", e))?;
    Ok(())
}

pub fn insert<K, V>(key: K, value: V, exp: Expiration) -> Result<()>
where
    K: Into<String>,
    V: Serialize + Encode + Sync + Send,
{
    let cache = CacheHand.get().ok_or_else(|| anyhow!("cache is null"))?;
    let k = key.into();
    let b = bincode::encode_to_vec(&value, config::standard())?;
    cache.insert(k, (exp, b));
    Ok(())
}

pub fn get<K, V>(key: K) -> Option<(Expiration, V)>
where
    K: Into<String>,
    V: DeserializeOwned + Decode<()> + Sync + Send,
{
    if let Some(h) = CacheHand.get() {
        let v = h.get(&key.into())?;
        let c = config::standard();
        let b = bincode::decode_from_slice::<V, _>(v.1.as_ref(), c);
        if let Ok((value, _)) = b {
            return Some((v.0, value));
        }
        if let Err(e) = b {
            log::error!("cache deserialize error: {}", e.to_string());
        }
        return None;
    }

    None
}

pub fn get_exp<K>(key: K) -> Option<Expiration>
where
    K: Into<String>,
{
    let value = CacheHand.get().map(|h| h.get(&key.into())).unwrap_or(None);
    if let Some(v) = value {
        return Some(v.0);
    }
    None
}

pub fn remove<K>(key: K)
where
    K: Into<String>,
{
    let k = key.into();
    CacheHand.get().map(|h| {
        h.invalidate(&k);
    });
}

pub fn contains_key<K>(key: K) -> bool
where
    K: Into<String>,
{
    let k = key.into();
    CacheHand.get().map(|h| h.contains_key(&k)).unwrap_or(false)
}

//每隔10检查缓存是否过期
pub fn check_exp_interval() {
    if let Some(cache) = CacheHand.get() {
        cache.run_pending_tasks();
    }
}

// 刷新key ttl
pub fn refresh<K>(key: K) -> Result<()>
where
    K: Into<String>,
{
    let k = key.into();
    let value = get(&k);

    let Some(value) = value else {
        return Err(anyhow!("key: {} not found", k));
    };

    if value.0 == Expiration::Never {
        return Ok(());
    }

    remove(&k);

    if let Some(c) = CacheHand.get() {
        c.insert(k, value);
    }

    Ok(())
}

#[cfg(test)]
#[allow(dead_code)]
mod test {

    use super::*;
    use std::thread::sleep;

    fn cache_key_expired(_key: Arc<String>, _: CacheData, _case: RemovalCause) {}
    fn init() {
        setup(Some(cache_key_expired), 512).unwrap();
    }

    #[test]
    fn test_encode_decode() {
        let value: i32 = 1000;
        let config = config::standard().with_little_endian();
        let b = bincode::encode_to_vec(&value, config).unwrap();
        println!("b-->{:?}", b);
        let (value, _) = bincode::decode_from_slice::<i32, _>(b.as_ref(), config).unwrap();
        println!("value-->{}", value);
    }

    #[test]
    fn test_cache_u16() {
        init();
        remove("test_cache_get_u1622");
        insert("test_cache_get_u1622", 1000, Expiration::Never).unwrap();
        let v = get::<_, u32>("test_cache_get_u1622");
        println!("test_cache_get_u1622-->{:?}", v);
    }

    #[test]
    fn test_cache_byte() {
        init();
        let b = b"hello world".to_vec();
        insert("test_cache_get_byte", b, Expiration::Never).unwrap();
        let v = get::<_, Vec<u8>>("test_cache_get_byte");
        println!("test_cache_get_byte-->{:?}", v);
    }

    #[test]
    fn test_cache_struct() {
        #[derive(Encode, Decode, Debug, Clone, Serialize, Deserialize)]
        struct Config {
            pub path: String,
            pub cache_capacity: u32,
            pub len: usize,
        }
        init();
        let b = Config {
            path: "test".to_string(),
            cache_capacity: 1024,
            len: 1024,
        };
        insert("test_cache_struct", b, Expiration::Never).unwrap();
        let v = get::<_, Config>("test_cache_struct");
        println!("test_cache_struct-->{:?}", v);
    }

    #[test]
    fn test_cache_get() {
        init();

        //
        insert("test_cache_get", "hello world", Expiration::Never).unwrap();
        let v = get::<_, String>("test_cache_get");
        println!("test_cache_get--->: {:?}", v);

        //
        insert("test_cache_get_bool", true, Expiration::Never).unwrap();
        let v = get::<_, bool>("test_cache_get_bool");
        println!("test_cache_get_bool-->{:?}", v);

        insert("test_cache_get_bool_false", false, Expiration::Never).unwrap();
        let v = get::<_, bool>("test_cache_get_bool_false");
        println!("test_cache_get_bool_false-->{:?}", v);

        //
        insert("test_cache_get_i32", 1000, Expiration::Never).unwrap();
        let v = get::<_, i32>("test_cache_get_i32");
        println!("test_cache_get_i32-->{:?}", v);

        //
        insert(
            "test_cache_get_byte",
            b"hello world".to_vec(),
            Expiration::Never,
        )
        .unwrap();
        let v = get::<_, Vec<u8>>("test_cache_get_byte");
        println!("test_cache_get_byte-->{:?}", v);
    }

    //
    fn test_src_mcache() {
        let expiry = CacheExpiry;

        let eviction_listener = |key, value, cause| {
            println!("过期 key-----> {key}. value--> {value:?}. Cause: {cause:?}");
        };

        let cache = Cache::builder()
            .max_capacity(100)
            .expire_after(expiry)
            .eviction_listener(eviction_listener)
            .build();

        cache.insert("0".to_string(), (Expiration::Second(5), b"a".to_vec()));
        cache.insert("1".to_string(), (Expiration::Second(6), b"b".to_vec()));
        cache.insert("2".to_string(), (Expiration::Never, b"c".to_vec()));
        cache.insert("3".to_string(), (Expiration::Never, b"3333".to_vec()));
        //update
        cache.insert(
            "0".to_string(),
            (Expiration::Second(10), b"abbbbaa".to_vec()),
        );

        println!("Entry count: {}", cache.entry_count());

        let Some(v) = cache.get("0") else {
            println!("cache.get(&0): none");
            return;
        };
        println!("cache.get(&0): {:?}", v);

        println!("0 {}", cache.contains_key("0"));
        println!("1 {}", cache.contains_key("1"));
        println!("2 {}", cache.contains_key("2"));
        println!("3 {}", cache.contains_key("3"));

        let re = cache.remove("3");
        println!("remove:{:?}", re);

        println!("\n Sleeping for 6 seconds...\n");
        sleep(Duration::from_secs(6));

        println!("Entry count: {}", cache.entry_count());
        println!("0 {}", cache.contains_key("0"));
        println!("1 {}", cache.contains_key("1"));
        println!("2 {}", cache.contains_key("2"));
        println!("3 {}", cache.contains_key("3"));

        let Some(v) = cache.get("2") else {
            return;
        };
        println!("cache.get(2): {:?}", v);
        sleep(Duration::from_secs(6));
    }

    fn test_cache_evication() {
        let _ = check_exp_interval();
        insert("check_evication", "过期了value", Expiration::Second(5)).unwrap();
        sleep(Duration::from_secs(3));
        println!("sleep 3s");

        let (_, v) = get::<_, String>("check_evication").unwrap();
        println!("get_string:{:?}", v);
        println!("sleep 3s");
        sleep(Duration::from_secs(3));
        println!("contains_key:{:?}", contains_key("check_evication"));
        sleep(Duration::from_secs(3));
        //println!("get_string:{:?}", get_string("check_evication"));
    }

    //
    fn test_cache_delete() {
        let key = "key_u64";
        // insert_u64("key_u64", 555, Expiration::Second(6));

        println!("sleep 3s");
        sleep(Duration::from_secs(3));
        println!("get_exp:{:?}", get_exp(key));
        //   println!("get_u64:{:?}", get_u64(&key));

        println!("update:");
        remove(key);
        sleep(Duration::from_secs(1));

        // insert_u64(key.to_string(), 666, Expiration::Second(12));
        // println!("get_exp:{:?}", get_exp(&key));
        // println!("get_u64:{:?}", get_u64(&key));

        // println!("sleep 3s");
        // sleep(Duration::from_secs(3));
        println!("get_exp:{:?}", get_exp(key));
        // println!("get_u64:{:?}", get_u64(&key));
    }

    //
    fn test_cache_refresh() {
        let key = "key_u64";
        //insert_u64("key_u64", 555, Expiration::Second(6));

        println!("sleep 3s");
        sleep(Duration::from_secs(3));
        let Some(exp_at) = get_exp(key) else {
            return;
        };
        println!("get_exp:{:?}", exp_at);
        //   println!("get_u64:{:?}", get_u64(&key));

        println!("del:");
        remove(key);

        // insert_u64(key, 666, exp_at);
        println!("get_exp:{:?}", get_exp(key));
        // println!("get_u64:{:?}", get_u64(&key));

        println!("sleep 3s");
        sleep(Duration::from_secs(2));
        println!("get_exp:{:?}", get_exp(key));
        //println!("get_u64:{:?}", get_u64(&key));

        println!("sleep 5s");
        sleep(Duration::from_secs(2));
        println!("get_exp:{:?}", get_exp(key));
        //  println!("get_u64:{:?}", get_u64(&key));
    }

    //
    fn test_cache_refresh2() {
        let key = "key_u64";
        //insert_u64(key, 555, Expiration::Second(4));

        println!("sleep 2s");
        sleep(Duration::from_secs(2));

        println!("refresh: ");
        refresh(key).expect("refresh error");

        println!("sleep 5s");
        sleep(Duration::from_secs(5));
        println!("get_exp:{:?}", get_exp(key));
        //  println!("get_u64:{:?}", get_u64(&key));
    }
}
