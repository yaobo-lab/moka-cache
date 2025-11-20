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
    Millis(u64),
    Second(u64),
    Minute(u64),
    Hour(u64),
}

impl Expiration {
    pub fn as_duration(&self) -> Option<Duration> {
        match self {
            Expiration::Never => None,
            Expiration::Millis(v) => Some(Duration::from_millis(v.clone())),
            Expiration::Second(v) => Some(Duration::from_secs(v.clone())),
            Expiration::Minute(v) => Some(Duration::from_secs(v.clone() * 60)),
            Expiration::Hour(v) => Some(Duration::from_secs(v.clone() * 60 * 60)),
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
        let k = key.into();

        let v = h.get(&k)?;

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
    if let Some(h) = CacheHand.get() {
        let k = key.into();
        let v = h.get(&k);
        let Some(v) = v else {
            return Err(anyhow!("key: {} not found", k));
        };

        if v.0 == Expiration::Never {
            return Ok(());
        }

        h.invalidate(&k);

        h.insert(k, v);

        return Ok(());
    }

    Err(anyhow!("cache is null"))
}

#[cfg(test)]
#[allow(dead_code)]
mod test {

    use super::*;
    use std::thread::sleep;

    fn cache_key_expired(key: Arc<String>, value: CacheData, cause: RemovalCause) {
        println!("过期 key-----> {key}. value--> {value:?}. Cause: {cause:?}");
    }
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

    #[test]
    fn test_cache_expire() {
        init();
        let key = "key_i32";
        insert("key_i32", 555, Expiration::Second(6)).unwrap();

        println!("sleep 3s");
        sleep(Duration::from_secs(3));
        let Some(exp_at) = get_exp(key) else {
            return;
        };
        println!("get_exp:{:?}", exp_at);
        let v = get::<_, i32>(key);
        println!("get_i32:{:?}", v);

        println!("sleep 3s");
        sleep(Duration::from_secs(2));
        println!("get_exp:{:?}", get_exp(key));

        println!("sleep 5s");
        sleep(Duration::from_secs(2));
        let v = get::<_, i32>(key);
        println!("get_i32:{:?}", v);

        let c = contains_key(key);
        println!("contains_key:{:?}", c);
    }

    #[test]
    fn test_cache_refresh() {
        init();
        let key = "key_i32".to_string();
        insert(&key, 555, Expiration::Second(6)).unwrap();
        let v = get::<_, i32>(&key);
        println!("get_i32:{:?}", v);

        sleep(Duration::from_secs(2));
        let Some(exp_at) = get_exp(&key) else {
            return;
        };
        println!("get_exp:{:?}", exp_at);

        if let Err(e) = refresh(&key) {
            println!("refresh error:{:?}", e);
            return;
        }
        println!("refresh get_exp:{:?}", get_exp(&key));

        println!("sleep 7s");
        sleep(Duration::from_secs(7));
        let v = get::<_, i32>(key);
        println!("get_i32:{:?}", v);
    }
}
