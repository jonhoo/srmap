#![feature(trivial_bounds)]
#![feature(extern_prelude)]

#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate serde;

pub mod srmap {
    use serde_json;
    use std::collections::HashMap;
    use std::hash::Hash;
    use std::char;
    use std::borrow::Borrow;
    use std::sync::{Arc, RwLock};
    use serde::ser::{Serialize, Serializer, SerializeStruct};

    #[derive(Clone)]
    #[derive(Deserialize, Debug)]
    pub struct SRMap<K, V, M>
    where
        K: Eq + Hash + Clone + std::fmt::Debug,
        V: Clone + Eq,
    {
        pub g_map: HashMap<K, Vec<V>>, // Global map
        pub b_map: HashMap<K, Vec<bool>>, // Auxiliary bit map for global map
        pub u_map: HashMap<(String, K), Vec<V>>, // Universe specific map (used only when K,V conflict with g_map)
        pub id_store: HashMap<usize, usize>,
        largest: usize,
        pub meta: M,
    }

    impl<K, V, M> SRMap<K, V, M>
    where
        K: Eq + Hash + Clone + std::fmt::Debug + serde::Serialize + serde::de::DeserializeOwned,
        V: Clone + Eq + serde::Serialize + serde::de::DeserializeOwned,
        M: serde::Serialize + serde::de::DeserializeOwned,
    {

        pub fn new(init_m: M) -> SRMap<K, V, M> {
            SRMap {
                g_map: HashMap::new(),
                b_map: HashMap::new(),
                u_map: HashMap::new(),
                id_store: HashMap::new(),
                largest: 0,
                meta: init_m
            }
        }

        pub fn insert(&mut self, k: K, v: Vec<V>, uid: usize){
            match self.id_store.get(&uid.clone()) {

            }
            // check if record is in the global map
            if self.g_map.contains_key(&k) {
                match self.g_map.get_mut(&k) {
                    Some(val) => {
                        // if it *is* in the global map, and the values match, update access for this user
                        if *val == v {
                            // update flag in global bit map for this user
                            match self.b_map.get_mut(&k) {
                                Some(mut bitmap) => {
                                    match self.id_store.get(&uid) {
                                        Some(&id) => {
                                            bitmap[id] = true;
                                        },
                                        None => {}
                                    }
                                },
                                None => {}
                            }
                        }
                        else {
                        // if v is different, insert (k,v) into umap as ('uid:k',v)
                            let uid_str = char::from_digit(uid as u32, 10).unwrap().to_string();
                            //let k_str: String = String::from(k).to_owned();
                            let key = (uid_str, k.clone());
                            self.u_map.insert(key, v.clone());
                        }
                    },
                    // add record to global map if it isn't already there
                    None => {}
                }
            } else {
                self.g_map.insert(k.clone(), v.clone());
                let mut bit_map = Vec::new();
                let user_index = self.id_store.entry(uid).or_insert(0);

                for x in 0..self.largest+1 {
                    if x != *user_index {
                        bit_map.push(false);
                    } else {
                        bit_map.push(true);
                    }
                }
                self.b_map.insert(k.clone(), bit_map);
            }
        }

        pub fn get(&self, k: &K, uid: usize) -> Option<&Vec<V>> {
            let uid_str = char::from_digit(uid as u32, 10).unwrap().to_string();
            //let uid_str: String =  String::from(uid).to_owned();
            // let k_str: String = String::from(k.clone()).to_owned();
            // let first_check = format!("{}{}", uid_str, k_str);
            let key = (uid_str, k.clone());
            match self.u_map.get(&key) {
               Some(val) => {Some(&val)},
               _ => {match self.g_map.get(&k) {
                        Some(g_val) => {
                            match self.b_map.get(&k) {
                                Some(bitmap) => {
                                    match self.id_store.get(&uid) {
                                        Some(&id) => {
                                            let accessible = bitmap[id];
                                            if accessible {
                                                let return_val = Some(g_val);
                                                return return_val;
                                            }
                                            else {
                                                return None;
                                            }
                                        },
                                        None => {None}
                                    }
                                },
                                None => {
                                    None
                                }
                            }
                        },
                        _ => {
                            None
                        }
                     }
                 }
             }
        }

        pub fn remove(&mut self, k: K, uid: usize) {
            println!("in remove! k: {:?}, uid: {:?}", k.clone(), uid.clone());
            let uid_str = char::from_digit(uid as u32, 10).unwrap().to_string();
            // let k_str: String = String::from(k.clone()).to_owned();
            // let first_check = format!("{}{}", uid_str, k_str);

            let key = (uid_str, k.clone());
            let mut remove_entirely = true;
            let mut hit_inner = false;

            if self.u_map.contains_key(&key) {
                self.u_map.remove(&key);
            }

            if self.g_map.contains_key(&k){
                match self.b_map.get_mut(&k){
                    Some(bitmap) => {
                        match self.id_store.get(&uid) {
                            Some(&id) => {
                                println!("here...");
                                bitmap[id] = false;
                                hit_inner = true;

                                for pt in bitmap {
                                    if *pt {
                                        remove_entirely = false;
                                    }
                                }
                            },
                            None => {}
                        }
                    }
                    None => {}
                }
            }

            if remove_entirely && hit_inner {
                self.g_map.remove(&k);
                self.b_map.remove(&k);
            }
        }

        pub fn add_user(&mut self, uid: usize) {
            self.largest = self.largest + 1;
            self.id_store.insert(uid.clone(), self.largest.clone());
            // add bitmap flag for this user in every global bitmap
            for (_, bmap) in self.b_map.iter_mut() {
                bmap.push(false);
            }
        }

        pub fn remove_user(&mut self, uid: usize) {
            let mut keys_to_del = Vec::new();
            // remove all u_map records for this user and revoke access from all global entries
            match self.id_store.get(&uid) {
                Some(&id) => {
                    for (k, bmap) in self.b_map.iter_mut() {
                        bmap[id] = false;

                        // do some cleanup: delete record if no users access it anymore
                        let mut delete_whole = true;
                        for flag in bmap.iter() {
                            if *flag {
                                delete_whole = false;
                            }
                        }
                        if delete_whole {
                            keys_to_del.push(k.clone());
                        }
                    }
                },
                None => {}
            }

            for k in &keys_to_del {
                self.g_map.remove(k);
                self.b_map.remove(k);
            }

            // remove all umap keys that start with this id
        }
    }

    impl<K, V, M> Serialize for SRMap<K, V, M>
    where
        K: Eq + Hash + Clone + std::fmt::Debug + serde::Serialize + serde::de::DeserializeOwned,
        V: Clone + Eq + serde::Serialize + serde::de::DeserializeOwned,
        M: serde::Serialize + serde::de::DeserializeOwned,
    {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let mut state = serializer.serialize_struct("SRMap", 6)?;
            state.serialize_field("g_map", &self.g_map)?;
            state.serialize_field("b_map", &self.b_map)?;
            state.serialize_field("u_map", &self.u_map)?;
            state.serialize_field("id_store", &self.id_store)?;
            state.serialize_field("largest", &self.largest)?;
            state.serialize_field("meta", &self.meta)?;
            state.end()
        }
    }

    // use serde::de::{self, Deserialize, Deserializer, Visitor, SeqAccess, MapAccess};
    //
    // impl<K, V, M, 'de> Deserialize<K, V, M, 'de> for SRMap<K, V, M> {
    //     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    //     where
    //         K: Eq + Hash + Clone + std::fmt::Debug + serde::Serialize,
    //         V: Clone + Eq + serde::Serialize,
    //         M: serde::Serialize,
    //     {
    //         enum Field { Secs, Nanos };
    //
    //         // This part could also be generated independently by:
    //         //
    //         //    #[derive(Deserialize)]
    //         //    #[serde(field_identifier, rename_all = "lowercase")]
    //         //    enum Field { Secs, Nanos }
    //         impl<'de> Deserialize<'de> for Field {
    //             fn deserialize<D>(deserializer: D) -> Result<Field, D::Error>
    //             where
    //                 D: Deserializer<'de>,
    //             {
    //                 struct FieldVisitor;
    //
    //                 impl<'de> Visitor<'de> for FieldVisitor {
    //                     type Value = Field;
    //
    //                     fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    //                         formatter.write_str("`secs` or `nanos`")
    //                     }
    //
    //                     fn visit_str<E>(self, value: &str) -> Result<Field, E>
    //                     where
    //                         E: de::Error,
    //                     {
    //                         match value {
    //                             "secs" => Ok(Field::Secs),
    //                             "nanos" => Ok(Field::Nanos),
    //                             _ => Err(de::Error::unknown_field(value, FIELDS)),
    //                         }
    //                     }
    //                 }
    //
    //                 deserializer.deserialize_identifier(FieldVisitor)
    //             }
    //         }
    //
    //         struct DurationVisitor;
    //
    //         impl<'de> Visitor<'de> for DurationVisitor {
    //             type Value = Duration;
    //
    //             fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    //                 formatter.write_str("struct Duration")
    //             }
    //
    //             fn visit_seq<V>(self, mut seq: V) -> Result<Duration, V::Error>
    //             where
    //                 V: SeqAccess<'de>,
    //             {
    //                 let secs = seq.next_element()?
    //                     .ok_or_else(|| de::Error::invalid_length(0, &self))?;
    //                 let nanos = seq.next_element()?
    //                     .ok_or_else(|| de::Error::invalid_length(1, &self))?;
    //                 Ok(Duration::new(secs, nanos))
    //             }
    //
    //             fn visit_map<V>(self, mut map: V) -> Result<Duration, V::Error>
    //             where
    //                 V: MapAccess<'de>,
    //             {
    //                 let mut secs = None;
    //                 let mut nanos = None;
    //                 while let Some(key) = map.next_key()? {
    //                     match key {
    //                         Field::Secs => {
    //                             if secs.is_some() {
    //                                 return Err(de::Error::duplicate_field("secs"));
    //                             }
    //                             secs = Some(map.next_value()?);
    //                         }
    //                         Field::Nanos => {
    //                             if nanos.is_some() {
    //                                 return Err(de::Error::duplicate_field("nanos"));
    //                             }
    //                             nanos = Some(map.next_value()?);
    //                         }
    //                     }
    //                 }
    //                 let secs = secs.ok_or_else(|| de::Error::missing_field("secs"))?;
    //                 let nanos = nanos.ok_or_else(|| de::Error::missing_field("nanos"))?;
    //                 Ok(Duration::new(secs, nanos))
    //             }
    //         }
    //
    //         const FIELDS: &'static [&'static str] = &["secs", "nanos"];
    //         deserializer.deserialize_struct("Duration", FIELDS, DurationVisitor)
    //     }
    // }

    #[derive(Deserialize)]
    #[derive(Serialize)]
    #[derive(Debug)]
    #[derive(Clone)]
    pub struct WriteHandle<K, V, M = ()>
    where
        K: Eq + Hash + Clone + std::fmt::Debug,
        V: Clone + Eq,
   {
       handle: Arc<RwLock<SRMap<K, V, M>>>,
   }

   pub fn new_write<K, V, M>(
       lock: Arc<RwLock<SRMap<K, V, M>>>,
   ) -> WriteHandle<K, V, M>
   where
       K: Eq + Hash + Clone + std::fmt::Debug + serde::Serialize + serde::de::DeserializeOwned,
       V: Clone + Eq + serde::Serialize + serde::de::DeserializeOwned,
       M: serde::Serialize + serde::de::DeserializeOwned,
    {
        WriteHandle {
            handle: lock,
        }
    }

    impl<K, V, M> WriteHandle<K, V, M>
    where
        K: Eq + Hash + Clone + std::fmt::Debug + serde::Serialize + serde::de::DeserializeOwned,
        V: Clone + Eq + serde::Serialize + serde::de::DeserializeOwned,
        M: Clone + serde::Serialize + serde::de::DeserializeOwned,
   {
       // Add the given value to the value-set of the given key.
       pub fn insert(&mut self, k: K, v: V, uid: usize) {
           let mut container = Vec::new();
           container.push(v);
           let mut w_handle = self.handle.write().unwrap();
           w_handle.insert(k.clone(), container, uid.clone());
       }

       // Replace the value-set of the given key with the given value.
       pub fn update(&mut self, k: K, v: V, uid: usize) {
           let mut container = Vec::new();
           container.push(v);
           let mut w_handle = self.handle.write().unwrap();
           w_handle.insert(k.clone(), container, uid.clone());
       }

       // Remove the given value from the value-set of the given key.
       pub fn remove(&mut self, k: K, uid: usize) {
           let mut w_handle = self.handle.write().unwrap();
           w_handle.remove(k.clone(), uid.clone());
       }

       pub fn add_user(&mut self, uid: usize) {
           let mut w_handle = self.handle.write().unwrap();
           w_handle.add_user(uid.clone());
       }

       pub fn remove_user(&mut self, uid: usize) {
           let mut w_handle = self.handle.write().unwrap();
           w_handle.remove_user(uid.clone());
       }

       pub fn refresh() {
           return
       }

       pub fn empty(&mut self, k: K, uid: usize) {
           let mut w_handle = self.handle.write().unwrap();
           w_handle.remove(k.clone(), uid.clone());
       }

       pub fn clear(&mut self, k: K, uid: usize) {
           let mut w_handle = self.handle.write().unwrap();
           w_handle.remove(k.clone(), uid.clone());
       }

       pub fn empty_at_index(&mut self, k: K, uid: usize) {
           let mut w_handle = self.handle.write().unwrap();
           w_handle.remove(k.clone(), uid.clone());
       }

       pub fn meta_get_and<F, T>(&self, key: &K, then: F, uid: usize) -> Option<(Option<T>, M)>
       where
           K: Hash + Eq,
           F: FnOnce(&[V]) -> T,
       {
           let r_handle = self.handle.read().unwrap();
           Some((r_handle.get(key, uid).map(move |v| then(&**v)), r_handle.meta.clone()))

       }

       pub fn is_empty(&self) -> bool {
           let r_handle = self.handle.read().unwrap();
           r_handle.g_map.is_empty()
       }

   }

   /// A handle that may be used to read from the SRMap.
   #[derive(Serialize, Deserialize, Debug, Clone)]
   pub struct ReadHandle<K, V, M = ()>
   where
       K: Eq + Hash + Clone + std::fmt::Debug,
       V: Clone + Eq,
    {
        pub(crate) inner: Arc<RwLock<SRMap<K, V, M>>>,
    }


   //  impl<K, V, M> Clone for ReadHandle<K, V, M>
   //  where
   //      K: Eq + Hash + std::fmt::Debug + Clone,
   //      V: Eq + Clone,
   //      M: 'static + Clone,
   // {
   //     fn clone(&self) -> Self {
   //         ReadHandle {
   //             inner: self.inner.clone()
   //         }
   //     }
   // }

   pub fn new_read<K, V, M>(store: Arc<RwLock<SRMap<K, V, M>>>) -> ReadHandle<K, V, M>
   where
       K: Eq + Hash + Clone + std::fmt::Debug + serde::Serialize + serde::de::DeserializeOwned,
       V: Clone + Eq + serde::Serialize + serde::de::DeserializeOwned,
       M: serde::Serialize + serde::de::DeserializeOwned,
    {
        ReadHandle {
            inner: store,
        }
    }

    impl<K, V, M> ReadHandle<K, V, M>
    where
        K: Eq + Hash + Clone + std::fmt::Debug + serde::Serialize + serde::de::DeserializeOwned,
        V: Clone + Eq + serde::Serialize + serde::de::DeserializeOwned,
        M: Clone + serde::Serialize + serde::de::DeserializeOwned,
   {
       pub fn g_map_diagnostics(&mut self) -> HashMap<K, Vec<V>> {
           let r_handle = self.inner.read().unwrap();
           r_handle.g_map.clone()
       }

       pub fn b_map_diagnostics(&mut self) -> HashMap<K, Vec<bool>> {
           let r_handle = self.inner.read().unwrap();
           r_handle.b_map.clone()
       }

       pub fn u_map_diagnostics(&mut self) -> HashMap<(String, K), Vec<V>> {
           let r_handle = self.inner.read().unwrap();
           r_handle.u_map.clone()
       }

       pub fn id_store_diagnostics(&mut self) -> HashMap<usize, usize> {
           let r_handle = self.inner.read().unwrap();
           r_handle.id_store.clone()
       }

       /// Get the current meta value.
       pub fn meta(&self) -> Option<M> {
          self.with_handle(|inner| inner.meta.clone())
       }

       /// Applies a function to the values corresponding to the key, and returns the result.
       pub fn get_lock(&self) -> Arc<RwLock<SRMap<K, V, M>>>
       {
           self.inner.clone() // TODO make sure this is valid! want to keep only one locked map
       }

       /// Returns the number of non-empty keys present in the map.
       pub fn len(&self) -> usize {
           let r_handle = self.inner.read().unwrap();
           r_handle.g_map.len()
       }

       /// Returns true if the map contains no elements.
       pub fn is_empty(&self) -> bool {
           let r_handle = self.inner.read().unwrap();
           r_handle.g_map.is_empty()
       }

       /// Applies a function to the values corresponding to the key, and returns the result.
       pub fn get_and<F, T>(&self, key: &K, then: F, uid: usize) -> Option<T>
       where
           K: Hash + Eq,
           F: FnOnce(&[V]) -> T,
       {
           let r_handle = self.inner.read().unwrap();
           r_handle.get(key, uid).map(move |v| then(&**v))
       }

       pub fn meta_get_and<F, T>(&self, key: &K, then: F, uid: usize) -> Option<(Option<T>, M)>
       where
           K: Hash + Eq,
           F: FnOnce(&[V]) -> T,
       {
           let r_handle = self.inner.read().unwrap();
           Some((r_handle.get(key, uid).map(move |v| then(&**v)), r_handle.meta.clone()))

       }

       fn with_handle<F, T>(&self, f: F) -> Option<T>
       where
          F: FnOnce(&SRMap<K, V, M>) -> T,
       {
           let r_handle = &*self.inner.read().unwrap();
           let res = Some(f(&r_handle));
           res
       }

       /// Read all values in the map, and transform them into a new collection.
       pub fn for_each<F>(&self, mut f: F)
       where
           F: FnMut(&K, &[V]),
       {
           self.with_handle(move |r_handle| {
            for (k, vs) in &r_handle.g_map {
                f(k, &vs[..])
            }
        });
       }

       pub fn contains_key<Q: ?Sized>(&self, key: &Q) -> bool
       where
           K: Borrow<Q>,
           Q: Hash + Eq,
       {
           let r_handle = self.inner.read().unwrap();
           r_handle.g_map.contains_key(key)
       }
   }

   pub fn construct<K, V, M>(meta_init: M) -> (ReadHandle<K, V, M>, WriteHandle<K, V, M>)
   where
       K: Eq + Hash + Clone + std::fmt::Debug + serde::Serialize + serde::de::DeserializeOwned,
       V: Clone + Eq + serde::Serialize + serde::de::DeserializeOwned,
       M: Clone + serde::Serialize + serde::de::DeserializeOwned,
    {
        let locked_map = Arc::new(RwLock::new(SRMap::<K,V,M>::new(meta_init)));
        let r_handle = new_read(locked_map);
        let lock = r_handle.get_lock();
        let w_handle = new_write(lock);
        //let gmap1 = lock.read().unwrap();
        (r_handle, w_handle)
    }
}
