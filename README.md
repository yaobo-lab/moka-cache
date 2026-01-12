## Install

```bash
cargo add moka-cache
```

## Usage

```rust
   
mod test {
    use super::*;
    use serde::{Deserialize, Serialize};
    use std::thread::sleep;

    fn cache_key_expired(key: Arc<String>, value: MokaCacheData, cause: RemovalCause) {
        println!("过期 key-----> {key}. value--> {value:?}. Cause: {cause:?}");
    }
    fn new() -> MokaCacheHandler {
        Arc::new(MokaCache::new_default(Some(cache_key_expired), 512))
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
```
