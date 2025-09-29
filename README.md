## Install

```bash
cargo add moka-cache
```

## Usage

```rust
   fn cache_key_expired(_key: Arc<String>, _: CacheData, _case: RemovalCause) {}
    fn init() {
        setup(Some(cache_key_expired), 512).unwrap();
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
        #[derive(Debug, Clone, Serialize, Deserialize)]
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

        //
        insert("test_cache_get_u16", 1200, Expiration::Never).unwrap();
        let v = get::<_, u16>("test_cache_get_u16");
        println!("test_cache_get_u16-->{:?}", v);

        //
        insert("test_cache_get_byte", b"hello world", Expiration::Never).unwrap();
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
```
