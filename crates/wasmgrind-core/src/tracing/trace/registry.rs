use std::{
    collections::{BinaryHeap, HashMap, binary_heap::PeekMut},
    fs::File,
    io::{BufReader, Read},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use anyhow::Error;
use bitcode::Buffer;

use crate::tracing::trace::{EventRecord, cursor::Cursor};

pub struct CacheFile {
    path: PathBuf,
    target_size: u64,
    file_id: u64,
}

impl CacheFile {
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    pub fn target_size(&self) -> u64 {
        self.target_size
    }

    pub fn id(&self) -> u64 {
        self.file_id
    }

    fn iter(&self) -> Result<CacheFileIter, Error> {
        let mut cache_file = BufReader::new(File::open(self.path())?);
        let mut u64_buf = [0_u8; std::mem::size_of::<u64>()];
        cache_file.read_exact(&mut u64_buf)?;
        let n_chunks = u64::from_le_bytes(u64_buf);

        let mut bitcode = Buffer::new();
        let iter = if n_chunks > 0 {
            // Get length of first chunk
            cache_file.read_exact(&mut u64_buf)?;
            let chunk_len = usize::try_from(u64::from_le_bytes(u64_buf))
                .expect("Buffer size needs to fit into usize");

            // Get first chunk
            let mut read_buf = vec![0_u8; chunk_len];
            cache_file.read_exact(&mut read_buf)?;
            let mut first_chunk: Vec<EventRecord> = bitcode.decode(&read_buf)?;
            first_chunk.reverse();

            CacheFileIter {
                file_id: self.id(),
                n_chunks,
                cache_file,
                current_chunk: first_chunk,
                bitcode,
                read_buf,
                idx: 0,
            }
        } else {
            CacheFileIter {
                file_id: self.id(),
                n_chunks,
                cache_file,
                current_chunk: Vec::new(),
                bitcode,
                read_buf: Vec::new(),
                idx: 0,
            }
        };

        Ok(iter)
    }
}

struct CacheFileIter {
    file_id: u64,
    n_chunks: u64,
    cache_file: BufReader<File>,
    current_chunk: Vec<EventRecord>,
    bitcode: Buffer,
    read_buf: Vec<u8>,
    idx: u64,
}

impl CacheFileIter {
    fn file_id(&self) -> u64 {
        self.file_id
    }
}

impl Cursor for CacheFileIter {
    type Item = EventRecord;
    type Meta = ();

    fn peek(&self) -> Option<(&Self::Item, Self::Meta)> {
        self.current_chunk.last().map(|item| (item, ()))
    }

    fn advance(&mut self) -> Result<Option<(Self::Item, Self::Meta)>, Error> {
        let item = self.current_chunk.pop();

        while self.current_chunk.is_empty() && self.idx + 1 < self.n_chunks {
            // Get length of next chunk
            let mut u64_buf = [0_u8; std::mem::size_of::<u64>()];
            self.cache_file.read_exact(&mut u64_buf)?;
            let chunk_len = usize::try_from(u64::from_le_bytes(u64_buf))
                .expect("Buffer size needs to fit into usize");

            // Get next chunk
            self.read_buf.resize(chunk_len, 0);
            self.cache_file.read_exact(&mut self.read_buf)?;
            let mut event_buffer: Vec<EventRecord> = self.bitcode.decode(&self.read_buf)?;
            event_buffer.reverse();

            // Update internal state
            self.current_chunk = event_buffer;
            self.idx += 1;
        }

        Ok(item.map(|record| (record, ())))
    }
}

struct ThreadFileIter<'a> {
    /// The files created by one thread
    files: Vec<&'a CacheFile>,
    /// The current iterator
    current: Option<CacheFileIter>,
}

impl<'a> Cursor for ThreadFileIter<'a> {
    type Item = EventRecord;
    type Meta = u64;

    fn peek(&self) -> Option<(&Self::Item, Self::Meta)> {
        self.current
            .as_ref()
            .and_then(|cursor| cursor.peek().map(|(record, _)| (record, cursor.file_id())))
        // Consider expecting an item here.
        // Our advance logic should ensure that either
        // self.current = None or an item exists
    }

    fn advance(&mut self) -> Result<Option<(Self::Item, Self::Meta)>, Error> {
        // Advance the current iterator if we have one
        if let Some(file_iter) = &mut self.current {
            let item = file_iter
                .advance()?
                .map(|(record, _)| (record, file_iter.file_id()));

            // If our current iterator is exhaused after the increment,
            // check if there are files left that contain records
            // and update the current iterator accordingly
            while file_iter.peek().is_none() {
                if let Some(file) = self.files.pop() {
                    *file_iter = file.iter()?;
                } else {
                    self.current = None;
                    break;
                }
            }

            Ok(item)
        } else {
            Ok(None)
        }
    }
}

pub struct Registry {
    next_file_id: u64,
    file_size: u64,
    registry_dir: PathBuf,
    cache: HashMap<u32, Vec<Arc<CacheFile>>>,
}

impl Registry {
    const CACHE_FILE_SIZE: u64 = 4 * 1024 * 1024 * 1024; // 4 GiB

    fn new<P: AsRef<Path>>(cache_dir: P) -> Result<Self, Error> {
        let registry_dir = cache_dir.as_ref().to_path_buf();
        if !std::fs::exists(&registry_dir)? {
            std::fs::create_dir_all(&registry_dir)?;
        }

        Ok(Self {
            next_file_id: 0,
            file_size: Self::CACHE_FILE_SIZE,
            registry_dir,
            cache: HashMap::new(),
        })
    }

    fn request_cache_file(&mut self, thread_id: u32) -> Arc<CacheFile> {
        let path = self
            .registry_dir
            .join(format!("cache-file-{}.data", self.next_file_id));
        let file = Arc::new(CacheFile {
            path,
            target_size: self.file_size,
            file_id: self.next_file_id,
        });

        self.cache.entry(thread_id).or_default().push(file.clone());

        self.next_file_id += 1;

        file
    }

    pub fn iter(&self) -> Result<RegistryIter<'_>, Error> {
        let streams = self
            .cache
            .values()
            .map(|thread_files| {
                let mut files: Vec<&CacheFile> =
                    thread_files.iter().map(|file| &**file).rev().collect();

                let current = files.pop().map(|file| file.iter()).transpose()?;

                Ok(ThreadFileIter { files, current })
            })
            .collect::<Result<Vec<ThreadFileIter<'_>>, Error>>()?;

        let heap = BinaryHeap::from_iter(streams.into_iter().filter_map(|src| {
            let (record, file_id) = src.peek()?;
            Some(HeapItem {
                key: record.id,
                file_id,
                src,
            })
        }));

        Ok(RegistryIter { heap })
    }
}

struct HeapItem<'a> {
    key: u64,
    file_id: u64,
    src: ThreadFileIter<'a>,
}

impl<'a> PartialEq for HeapItem<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}

impl<'a> Eq for HeapItem<'a> {}

impl<'a> PartialOrd for HeapItem<'a> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<'a> Ord for HeapItem<'a> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // The order is as follows:
        // - iterators with smaller keys are greater than ones with bigger keys
        //   => Reverse(key 1 < key 2) => Reverse(key 1 cmp key 2)
        // - iterators with the same keys are compared via their file id
        //   => Higher file id means newer file => is greater
        self.key
            .cmp(&other.key)
            .reverse()
            .then(self.file_id.cmp(&other.file_id))
    }
}

pub struct RegistryIter<'a> {
    heap: BinaryHeap<HeapItem<'a>>,
}

impl<'a> RegistryIter<'a> {
    fn try_next(&mut self) -> Result<Option<EventRecord>, Error> {
        if let Some(HeapItem {
            key,
            file_id: _,
            src: mut current,
        }) = self.heap.pop()
        {
            while let Some(mut heap_top) = self.heap.peek_mut()
                && heap_top.key == key
            {
                loop {
                    // Skip all values on top of the heap that have the same key
                    // as the current value (do not yield duplicates!)
                    let item = heap_top.src.advance()?;
                    debug_assert_eq!(
                        item.map(|(record, file_id)| (record.id, file_id)),
                        Some((heap_top.key, heap_top.file_id)),
                        "Expected current heap top value and yielded src value to be equal"
                    );
                    if let Some((record, file_id)) = heap_top.src.peek() {
                        if record.id == key {
                            continue;
                        } else {
                            heap_top.key = record.id;
                            heap_top.file_id = file_id;
                        }
                    } else {
                        // If the iterator is now done, remove it from the heap
                        PeekMut::pop(heap_top);
                    }

                    break;
                }
            }

            // Advance the current iterator
            let item = current.advance()?;

            // Push it back to the heap if it has records left
            if let Some((record, file_id)) = current.peek() {
                self.heap.push(HeapItem {
                    key: record.id,
                    file_id,
                    src: current,
                });
            }

            Ok(item.map(|(record, _)| record))
        } else {
            Ok(None)
        }
    }
}

impl<'a> Iterator for RegistryIter<'a> {
    type Item = Result<EventRecord, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        self.try_next().transpose()
    }
}

pub struct TraceRegistry(Mutex<Registry>);

impl TraceRegistry {
    pub fn new<P: AsRef<Path>>(cache_dir: P) -> Result<Self, Error> {
        Ok(Self(Mutex::new(Registry::new(cache_dir)?)))
    }

    pub fn request_cache_file(&self, thread_id: u32) -> Arc<CacheFile> {
        self.0
            .lock()
            .expect("TraceRegistry lock was poisoned!")
            .request_cache_file(thread_id)
    }

    pub fn close(self) -> Registry {
        self.0
            .into_inner()
            .expect("Trace registry mutex was poisoned")
    }
}
