//! A LPM Trie.
use std::{
    borrow::Borrow,
    convert::{AsMut, AsRef},
    marker::PhantomData,
};

use crate::{
    maps::{check_kv_size, IterableMap, MapData, MapError, MapIter, MapKeys},
    sys::{bpf_map_delete_elem, bpf_map_get_next_key, bpf_map_lookup_elem, bpf_map_update_elem},
    Pod,
};

/// A Longest Prefix Match Trie.
///
/// # Minimum kernel version
///
/// The minimum kernel version required to use this feature is 4.20.
///
/// # Examples
///
/// ```no_run
/// # let mut bpf = aya::Bpf::load(&[])?;
/// use aya::maps::lpm_trie::{LpmTrie, Key};
/// use std::net::Ipv4Addr;
///
/// let mut trie = LpmTrie::try_from(bpf.map_mut("LPM_TRIE").unwrap())?;
/// let ipaddr = Ipv4Addr::new(8, 8, 8, 8);
/// // The following represents a key for the "8.8.8.8/16" subnet.
/// // The first argument - the prefix length - represents how many bytes should be matched against. The second argument is the actual data to be matched.
/// let key = Key::new(16, u32::from(ipaddr).to_be());
/// trie.insert(&key, 1, 0)?;
///
/// // LpmTrie matches against the longest (most accurate) key.
/// let lookup = Key::new(32, u32::from(ipaddr).to_be());
/// let value = trie.get(&lookup, 0)?;
/// assert_eq!(value, 1);
///
/// // If we were to insert a key with longer 'prefix_len'
/// // our trie should match against it.
/// let longer_key = Key::new(24, u32::from(ipaddr).to_be());
/// trie.insert(&longer_key, 2, 0)?;
/// let value = trie.get(&lookup, 0)?;
/// assert_eq!(value, 2);
/// # Ok::<(), aya::BpfError>(())
/// ```

#[doc(alias = "BPF_MAP_TYPE_LPM_TRIE")]
pub struct LpmTrie<T, K, V> {
    inner: T,
    _k: PhantomData<K>,
    _v: PhantomData<V>,
}

/// A Key for and LpmTrie map.
///
/// # Examples
///
/// ```no_run
/// use aya::maps::lpm_trie::{LpmTrie, Key};
/// use std::net::Ipv4Addr;
///
/// let ipaddr = Ipv4Addr::new(8,8,8,8);
/// let key =  Key::new(16, u32::from(ipaddr).to_be());
/// ```
#[repr(packed)]
pub struct Key<K: Pod> {
    /// Represents the number of bytes matched against.
    pub prefix_len: u32,
    /// Represents arbitrary data stored in the LpmTrie.
    pub data: K,
}

impl<K: Pod> Key<K> {
    /// Creates a new key.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use aya::maps::lpm_trie::{LpmTrie, Key};
    /// use std::net::Ipv4Addr;
    ///
    /// let ipaddr = Ipv4Addr::new(8, 8, 8, 8);
    /// let key =  Key::new(16, u32::from(ipaddr).to_be());
    /// ```
    pub fn new(prefix_len: u32, data: K) -> Self {
        Self { prefix_len, data }
    }
}

impl<K: Pod> Copy for Key<K> {}

impl<K: Pod> Clone for Key<K> {
    fn clone(&self) -> Self {
        *self
    }
}

// A Pod impl is required as Key struct is a key for a map.
unsafe impl<K: Pod> Pod for Key<K> {}

impl<T: AsRef<MapData>, K: Pod, V: Pod> LpmTrie<T, K, V> {
    pub(crate) fn new(map: T) -> Result<LpmTrie<T, K, V>, MapError> {
        let data = map.as_ref();
        check_kv_size::<Key<K>, V>(data)?;

        let _ = data.fd_or_err()?;

        Ok(LpmTrie {
            inner: map,
            _k: PhantomData,
            _v: PhantomData,
        })
    }

    /// Returns a copy of the value associated with the longest prefix matching key in the LpmTrie.
    pub fn get(&self, key: &Key<K>, flags: u64) -> Result<V, MapError> {
        let fd = self.inner.as_ref().fd_or_err()?;
        let value = bpf_map_lookup_elem(fd, key, flags).map_err(|(_, io_error)| {
            MapError::SyscallError {
                call: "bpf_map_lookup_elem".to_owned(),
                io_error,
            }
        })?;
        value.ok_or(MapError::KeyNotFound)
    }

    /// An iterator visiting all key-value pairs in arbitrary order. The
    /// iterator item type is `Result<(K, V), MapError>`.
    pub fn iter(&self) -> MapIter<'_, Key<K>, V, Self> {
        MapIter::new(self)
    }

    /// An iterator visiting all keys in arbitrary order. The iterator element
    /// type is `Result<Key<K>, MapError>`.
    pub fn keys(&self) -> MapKeys<'_, Key<K>> {
        MapKeys::new(self.inner.as_ref())
    }

    /// An iterator visiting all keys matching key. The
    /// iterator item type is `Result<Key<K>, MapError>`.
    pub fn iter_key(&self, key: Key<K>) -> LpmTrieKeys<'_, K> {
        LpmTrieKeys::new(self.inner.as_ref(), key)
    }
}

impl<T: AsMut<MapData>, K: Pod, V: Pod> LpmTrie<T, K, V> {
    /// Inserts a key value pair into the map.
    pub fn insert(
        &mut self,
        key: &Key<K>,
        value: impl Borrow<V>,
        flags: u64,
    ) -> Result<(), MapError> {
        let fd = self.inner.as_mut().fd_or_err()?;
        bpf_map_update_elem(fd, Some(key), value.borrow(), flags).map_err(|(_, io_error)| {
            MapError::SyscallError {
                call: "bpf_map_update_elem".to_owned(),
                io_error,
            }
        })?;

        Ok(())
    }

    /// Removes an element from the map.
    ///
    /// Both the prefix and data must match exactly - this method does not do a longest prefix match.
    pub fn remove(&mut self, key: &Key<K>) -> Result<(), MapError> {
        let fd = self.inner.as_mut().fd_or_err()?;
        bpf_map_delete_elem(fd, key)
            .map(|_| ())
            .map_err(|(_, io_error)| MapError::SyscallError {
                call: "bpf_map_delete_elem".to_owned(),
                io_error,
            })
    }
}

impl<T: AsRef<MapData>, K: Pod, V: Pod> IterableMap<Key<K>, V> for LpmTrie<T, K, V> {
    fn map(&self) -> &MapData {
        self.inner.as_ref()
    }

    fn get(&self, key: &Key<K>) -> Result<V, MapError> {
        self.get(key, 0)
    }
}

/// Iterator returned by `LpmTrie::iter_key()`.
pub struct LpmTrieKeys<'coll, K: Pod> {
    map: &'coll MapData,
    err: bool,
    key: Key<K>,
}

impl<'coll, K: Pod> LpmTrieKeys<'coll, K> {
    fn new(map: &'coll MapData, key: Key<K>) -> LpmTrieKeys<'coll, K> {
        LpmTrieKeys {
            map,
            err: false,
            key,
        }
    }
}

impl<K: Pod> Iterator for LpmTrieKeys<'_, K> {
    type Item = Result<Key<K>, MapError>;

    fn next(&mut self) -> Option<Result<Key<K>, MapError>> {
        if self.err {
            return None;
        }

        let fd = match self.map.fd_or_err() {
            Ok(fd) => fd,
            Err(e) => {
                self.err = true;
                return Some(Err(e));
            }
        };

        match bpf_map_get_next_key(fd, Some(&self.key)) {
            Ok(Some(key)) => {
                self.key = key;
                Some(Ok(key))
            }
            Ok(None) => None,
            Err((_, io_error)) => {
                self.err = true;
                Some(Err(MapError::SyscallError {
                    call: "bpf_map_get_next_key".to_owned(),
                    io_error,
                }))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        bpf_map_def,
        generated::{
            bpf_cmd,
            bpf_map_type::{BPF_MAP_TYPE_LPM_TRIE, BPF_MAP_TYPE_PERF_EVENT_ARRAY},
        },
        maps::{Map, MapData},
        obj::{
            self,
            maps::{LegacyMap, MapKind},
        },
        sys::{override_syscall, SysResult, Syscall},
    };
    use libc::{EFAULT, ENOENT};
    use std::{io, mem, net::Ipv4Addr};

    fn new_obj_map() -> obj::Map {
        obj::Map::Legacy(LegacyMap {
            def: bpf_map_def {
                map_type: BPF_MAP_TYPE_LPM_TRIE as u32,
                key_size: mem::size_of::<Key<u32>>() as u32,
                value_size: 4,
                max_entries: 1024,
                ..Default::default()
            },
            section_index: 0,
            symbol_index: 0,
            data: Vec::new(),
            kind: MapKind::Other,
        })
    }

    fn sys_error(value: i32) -> SysResult {
        Err((-1, io::Error::from_raw_os_error(value)))
    }

    #[test]
    fn test_wrong_key_size() {
        let map = MapData {
            obj: new_obj_map(),
            fd: None,
            pinned: false,
            btf_fd: None,
        };
        assert!(matches!(
            LpmTrie::<_, u16, u32>::new(&map),
            Err(MapError::InvalidKeySize {
                size: 6,
                expected: 8 // four bytes for prefixlen and four bytes for data.
            })
        ));
    }

    #[test]
    fn test_wrong_value_size() {
        let map = MapData {
            obj: new_obj_map(),
            fd: None,
            pinned: false,
            btf_fd: None,
        };
        assert!(matches!(
            LpmTrie::<_, u32, u16>::new(&map),
            Err(MapError::InvalidValueSize {
                size: 2,
                expected: 4
            })
        ));
    }

    #[test]
    fn test_try_from_wrong_map() {
        let map_data = MapData {
            obj: obj::Map::Legacy(LegacyMap {
                def: bpf_map_def {
                    map_type: BPF_MAP_TYPE_PERF_EVENT_ARRAY as u32,
                    key_size: 4,
                    value_size: 4,
                    max_entries: 1024,
                    ..Default::default()
                },
                section_index: 0,
                symbol_index: 0,
                data: Vec::new(),
                kind: MapKind::Other,
            }),
            fd: None,
            btf_fd: None,
            pinned: false,
        };

        let map = Map::PerfEventArray(map_data);

        assert!(matches!(
            LpmTrie::<_, u32, u32>::try_from(&map),
            Err(MapError::InvalidMapType { .. })
        ));
    }

    #[test]
    fn test_new_not_created() {
        let mut map = MapData {
            obj: new_obj_map(),
            fd: None,
            pinned: false,
            btf_fd: None,
        };

        assert!(matches!(
            LpmTrie::<_, u32, u32>::new(&mut map),
            Err(MapError::NotCreated { .. })
        ));
    }

    #[test]
    fn test_new_ok() {
        let mut map = MapData {
            obj: new_obj_map(),
            fd: Some(42),
            pinned: false,
            btf_fd: None,
        };

        assert!(LpmTrie::<_, u32, u32>::new(&mut map).is_ok());
    }

    #[test]
    fn test_try_from_ok() {
        let map_data = MapData {
            obj: new_obj_map(),
            fd: Some(42),
            pinned: false,
            btf_fd: None,
        };

        let map = Map::LpmTrie(map_data);
        assert!(LpmTrie::<_, u32, u32>::try_from(&map).is_ok())
    }

    #[test]
    fn test_insert_syscall_error() {
        override_syscall(|_| sys_error(EFAULT));

        let mut map = MapData {
            obj: new_obj_map(),
            fd: Some(42),
            pinned: false,
            btf_fd: None,
        };
        let mut trie = LpmTrie::<_, u32, u32>::new(&mut map).unwrap();
        let ipaddr = Ipv4Addr::new(8, 8, 8, 8);
        let key = Key::new(16, u32::from(ipaddr).to_be());
        assert!(matches!(
            trie.insert(&key, 1, 0),
            Err(MapError::SyscallError { call, io_error }) if call == "bpf_map_update_elem" && io_error.raw_os_error() == Some(EFAULT)
        ));
    }

    #[test]
    fn test_insert_ok() {
        override_syscall(|call| match call {
            Syscall::Bpf {
                cmd: bpf_cmd::BPF_MAP_UPDATE_ELEM,
                ..
            } => Ok(1),
            _ => sys_error(EFAULT),
        });

        let mut map = MapData {
            obj: new_obj_map(),
            fd: Some(42),
            pinned: false,
            btf_fd: None,
        };

        let mut trie = LpmTrie::<_, u32, u32>::new(&mut map).unwrap();
        let ipaddr = Ipv4Addr::new(8, 8, 8, 8);
        let key = Key::new(16, u32::from(ipaddr).to_be());
        assert!(trie.insert(&key, 1, 0).is_ok());
    }

    #[test]
    fn test_remove_syscall_error() {
        override_syscall(|_| sys_error(EFAULT));

        let mut map = MapData {
            obj: new_obj_map(),
            fd: Some(42),
            pinned: false,
            btf_fd: None,
        };
        let mut trie = LpmTrie::<_, u32, u32>::new(&mut map).unwrap();
        let ipaddr = Ipv4Addr::new(8, 8, 8, 8);
        let key = Key::new(16, u32::from(ipaddr).to_be());
        assert!(matches!(
            trie.remove(&key),
            Err(MapError::SyscallError { call, io_error }) if call == "bpf_map_delete_elem" && io_error.raw_os_error() == Some(EFAULT)
        ));
    }

    #[test]
    fn test_remove_ok() {
        override_syscall(|call| match call {
            Syscall::Bpf {
                cmd: bpf_cmd::BPF_MAP_DELETE_ELEM,
                ..
            } => Ok(1),
            _ => sys_error(EFAULT),
        });

        let mut map = MapData {
            obj: new_obj_map(),
            fd: Some(42),
            pinned: false,
            btf_fd: None,
        };
        let mut trie = LpmTrie::<_, u32, u32>::new(&mut map).unwrap();
        let ipaddr = Ipv4Addr::new(8, 8, 8, 8);
        let key = Key::new(16, u32::from(ipaddr).to_be());
        assert!(trie.remove(&key).is_ok());
    }

    #[test]
    fn test_get_syscall_error() {
        override_syscall(|_| sys_error(EFAULT));
        let map = MapData {
            obj: new_obj_map(),
            fd: Some(42),
            pinned: false,
            btf_fd: None,
        };
        let trie = LpmTrie::<_, u32, u32>::new(&map).unwrap();
        let ipaddr = Ipv4Addr::new(8, 8, 8, 8);
        let key = Key::new(16, u32::from(ipaddr).to_be());

        assert!(matches!(
            trie.get(&key, 0),
            Err(MapError::SyscallError { call, io_error }) if call == "bpf_map_lookup_elem" && io_error.raw_os_error() == Some(EFAULT)
        ));
    }

    #[test]
    fn test_get_not_found() {
        override_syscall(|call| match call {
            Syscall::Bpf {
                cmd: bpf_cmd::BPF_MAP_LOOKUP_ELEM,
                ..
            } => sys_error(ENOENT),
            _ => sys_error(EFAULT),
        });
        let map = MapData {
            obj: new_obj_map(),
            fd: Some(42),
            pinned: false,
            btf_fd: None,
        };
        let trie = LpmTrie::<_, u32, u32>::new(&map).unwrap();
        let ipaddr = Ipv4Addr::new(8, 8, 8, 8);
        let key = Key::new(16, u32::from(ipaddr).to_be());

        assert!(matches!(trie.get(&key, 0), Err(MapError::KeyNotFound)));
    }
}
