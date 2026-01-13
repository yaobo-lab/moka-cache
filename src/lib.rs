use anyhow::{anyhow, Result};
use bincode::config;
pub use moka;
use moka::{notification::RemovalCause, sync::Cache, Expiry};
use serde::{de::DeserializeOwned, Serialize};
use std::{
    sync::Arc,
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

pub type MokaCacheData = (Expiration, Vec<u8>);
pub struct MokaCache(pub Cache<String, (Expiration, Vec<u8>)>);
pub type MokaCacheHandler = Arc<MokaCache>;

pub struct MokaCacheExpiry;
impl Expiry<String, (Expiration, Vec<u8>)> for MokaCacheExpiry {
    fn expire_after_create(
        &self,
        _key: &String,
        value: &(Expiration, Vec<u8>),
        _current_time: Instant,
    ) -> Option<Duration> {
        value.0.as_duration()
    }
}

impl MokaCache {
    //初始化缓存
    pub fn new_default(
        callback: Option<fn(Arc<String>, MokaCacheData, RemovalCause)>,
        max_cap: u64,
    ) -> MokaCache {
        let mut c = Cache::builder()
            .max_capacity(max_cap)
            .expire_after(MokaCacheExpiry {});
        if let Some(callback) = callback {
            c = c.eviction_listener(callback);
        }
        MokaCache(c.build())
    }

    pub fn insert<K, V>(&self, key: K, value: V, exp: Expiration) -> Result<()>
    where
        K: AsRef<str>,
        V: Serialize + Sync + Send,
    {
        let k = key.as_ref();
        let b = bincode::serde::encode_to_vec(&value, config::standard())?;
        self.0.insert(k.into(), (exp, b));
        Ok(())
    }

    pub fn get<K, V>(&self, key: K) -> Option<(Expiration, V)>
    where
        K: AsRef<str>,
        V: DeserializeOwned + Sync + Send,
    {
        let v = self.0.get(key.as_ref())?;
        let c = config::standard();
        let b = bincode::serde::decode_from_slice::<V, _>(v.1.as_ref(), c);
        if let Ok((value, _)) = b {
            return Some((v.0, value));
        }
        if let Err(e) = b {
            log::error!("cache deserialize error: {}", e.to_string());
        }
        None
    }

    pub fn deserialize<V>(d: &[u8]) -> Option<V>
    where
        V: DeserializeOwned + Sync + Send,
    {
        let c = config::standard();
        let b = bincode::serde::decode_from_slice::<V, _>(d, c);
        if let Ok((value, _)) = b {
            return Some(value);
        }
        if let Err(e) = b {
            log::error!("deserialize error: {}", e.to_string());
        }
        return None;
    }

    pub fn get_exp<K>(&self, key: K) -> Option<Expiration>
    where
        K: AsRef<str>,
    {
        let value = self.0.get(key.as_ref());
        if let Some(v) = value {
            return Some(v.0);
        }
        None
    }

    pub fn remove<K>(&self, key: K)
    where
        K: AsRef<str>,
    {
        self.0.invalidate(key.as_ref());
    }

    pub fn contains_key<K>(&self, key: K) -> bool
    where
        K: AsRef<str>,
    {
        self.0.contains_key(key.as_ref())
    }

    //每隔10检查缓存是否过期
    pub fn check_exp_interval(&self) {
        self.0.run_pending_tasks();
    }

    // 刷新key ttl
    pub fn refresh<K>(&self, key: K) -> Result<()>
    where
        K: AsRef<str>,
    {
        let k = key.as_ref();
        let v = self.0.get(k);
        let Some(v) = v else {
            return Err(anyhow!("key: {} not found", k));
        };
        if v.0 == Expiration::Never {
            return Ok(());
        }
        self.0.invalidate(k);
        self.0.insert(k.into(), v);
        return Ok(());
    }
}

#[cfg(test)]
#[allow(dead_code)]
mod test {
    use super::*;
    use serde::{Deserialize, Serialize};
    use std::process;
    use std::sync::Mutex;
    use std::thread::{self, sleep};
    use toolkit_rs::logger::{self, LogConfig};

    static COUNT: Mutex<u32> = Mutex::new(0);
    fn cache_key_expired(key: Arc<String>, _value: MokaCacheData, cause: RemovalCause) {
        let mut k = 0;
        if let Ok(mut c) = COUNT.lock() {
            *c += 1;
            k = *c;
        }
        k = k - 1;
        log::debug!("{}. 过期 key-----> {key}. Cause: {cause:?}", k);
        // if RemovalCause::Expired == cause {
        //     log::debug!("过期 key-----> {key}. value--> {value:?}. Cause: {cause:?}");
        // }
    }
    fn new() -> MokaCacheHandler {
        let lcfg = LogConfig {
            style: logger::LogStyle::Default,
            ..LogConfig::default()
        };
        logger::setup(lcfg).unwrap_or_else(|e| {
            println!("log setup err:{}", e);
            process::exit(1);
        });

        Arc::new(MokaCache::new_default(Some(cache_key_expired), 512))
    }

    #[test]
    fn test_cache_callback() {
        let m = new();

        let key = "key_0001";

        let mclone = m.clone();
        let mut k = 0;
        thread::spawn(move || loop {
            thread::sleep(Duration::from_millis(50));
            mclone.check_exp_interval();
            k = k + 1;
            //log::debug!("{}.<---------------- check expire ---------------->", k);
        });

        for i in 0..10 {
            thread::sleep(Duration::from_millis(50));

            let c = m.contains_key(key);
            let g = m.get::<_, u32>(key);

            let mut v = 1;
            if let Some(k) = g {
                v = v + k.1;
            }
            m.remove(key);

            log::debug!("{}. key exists:{} , get value:{}", i, c, v);

            m.insert(key, v, Expiration::Millis(100)).unwrap();
        }

        thread::sleep(Duration::from_secs(2));
        let v = m.get::<_, u32>(key);
        log::debug!("ok test_cache_get_u1622-->{:?}", v);
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
        let m = new();
        m.remove("test_cache_get_u1622");
        m.insert("test_cache_get_u1622", 1000, Expiration::Never)
            .unwrap();
        let v = m.get::<_, u32>("test_cache_get_u1622");
        println!("test_cache_get_u1622-->{:?}", v);
    }

    #[test]
    fn test_cache_byte() {
        let m = new();
        let b = b"hello world".to_vec();
        m.insert("test_cache_get_byte", b, Expiration::Never)
            .unwrap();
        let v = m.get::<_, Vec<u8>>("test_cache_get_byte");
        println!("test_cache_get_byte-->{:?}", v);
    }

    #[test]
    fn test_cache_struct() {
        #[derive(Debug, Clone, Serialize, Deserialize)]
        struct Config {
            pub path: String,
            pub cache_capacity: u32,
            pub len: usize,
        }
        let m = new();
        let b = Config {
            path: "test".to_string(),
            cache_capacity: 1024,
            len: 1024,
        };
        m.insert("test_cache_struct", b, Expiration::Never).unwrap();

        let v = m.get::<_, Config>("test_cache_struct");
        println!("test_cache_struct-->{:?}", v);
    }

    #[test]
    fn test_cache_get() {
        let m = new();

        //
        m.insert("test_cache_get", "hello world", Expiration::Never)
            .unwrap();
        let v = m.get::<_, String>("test_cache_get");
        println!("test_cache_get--->: {:?}", v);

        //
        m.insert("test_cache_get_bool", true, Expiration::Never)
            .unwrap();
        let v = m.get::<_, bool>("test_cache_get_bool");
        println!("test_cache_get_bool-->{:?}", v);

        m.insert("test_cache_get_bool_false", false, Expiration::Never)
            .unwrap();
        let v = m.get::<_, bool>("test_cache_get_bool_false");
        println!("test_cache_get_bool_false-->{:?}", v);

        //
        m.insert("test_cache_get_i32", 1000, Expiration::Never)
            .unwrap();
        let v = m.get::<_, i32>("test_cache_get_i32");
        println!("test_cache_get_i32-->{:?}", v);

        //
        m.insert(
            "test_cache_get_byte",
            b"hello world".to_vec(),
            Expiration::Never,
        )
        .unwrap();
        let v = m.get::<_, Vec<u8>>("test_cache_get_byte");
        println!("test_cache_get_byte-->{:?}", v);
    }

    //
    fn test_cache_delete() {
        let m = new();
        let key = "key_u64";
        // insert_u64("key_u64", 555, Expiration::Second(6));

        println!("sleep 3s");
        sleep(Duration::from_secs(3));
        println!("get_exp:{:?}", m.get_exp(key));
        //   println!("get_u64:{:?}", get_u64(&key));

        println!("update:");
        m.remove(key);
        sleep(Duration::from_secs(1));

        // insert_u64(key.to_string(), 666, Expiration::Second(12));
        // println!("get_exp:{:?}", get_exp(&key));
        // println!("get_u64:{:?}", get_u64(&key));

        // println!("sleep 3s");
        // sleep(Duration::from_secs(3));
        println!("get_exp:{:?}", m.get_exp(key));
        // println!("get_u64:{:?}", get_u64(&key));
    }

    #[test]
    fn test_cache_expire() {
        let m = new();
        let key = "key_i32";
        m.insert("key_i32", 555, Expiration::Second(6)).unwrap();

        println!("sleep 3s");
        sleep(Duration::from_secs(3));
        let Some(exp_at) = m.get_exp(key) else {
            return;
        };
        println!("get_exp:{:?}", exp_at);
        let v = m.get::<_, i32>(key);
        println!("get_i32:{:?}", v);

        println!("sleep 3s");
        sleep(Duration::from_secs(2));
        println!("get_exp:{:?}", m.get_exp(key));

        println!("sleep 5s");
        sleep(Duration::from_secs(2));
        let v = m.get::<_, i32>(key);
        println!("get_i32:{:?}", v);

        let c = m.contains_key(key);
        println!("contains_key:{:?}", c);
    }

    #[test]
    fn test_cache_refresh() {
        let m = new();
        let key = "key_i32".to_string();
        m.insert(&key, 555, Expiration::Second(6)).unwrap();
        let v = m.get::<_, i32>(&key);
        println!("get_i32:{:?}", v);

        sleep(Duration::from_secs(2));
        let Some(exp_at) = m.get_exp(&key) else {
            return;
        };
        println!("get_exp:{:?}", exp_at);

        if let Err(e) = m.refresh(&key) {
            println!("refresh error:{:?}", e);
            return;
        }
        println!("refresh get_exp:{:?}", m.get_exp(&key));

        println!("sleep 7s");
        sleep(Duration::from_secs(7));
        let v = m.get::<_, i32>(key);
        println!("get_i32:{:?}", v);
    }
}
