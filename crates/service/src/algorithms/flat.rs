use super::quantization::Quantization;
use super::raw::Raw;
use crate::index::segments::growing::GrowingSegment;
use crate::index::segments::sealed::SealedSegment;
use crate::index::{IndexOptions, SearchOptions};
use crate::prelude::*;
use crate::utils::dir_ops::sync_dir;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::fs::create_dir;
use std::path::PathBuf;
use std::sync::Arc;

pub struct Flat<S: G> {
    mmap: FlatMmap<S>,
}

impl<S: G> Flat<S> {
    pub fn create(
        path: PathBuf,
        options: IndexOptions,
        sealed: Vec<Arc<SealedSegment<S>>>,
        growing: Vec<Arc<GrowingSegment<S>>>,
    ) -> Self {
        create_dir(&path).unwrap();
        let ram = make(path.clone(), sealed, growing, options.clone());
        let mmap = save(ram, path.clone());
        sync_dir(&path);
        Self { mmap }
    }
    pub fn open(path: PathBuf, options: IndexOptions) -> Self {
        let mmap = load(path, options.clone());
        Self { mmap }
    }

    pub fn len(&self) -> u32 {
        self.mmap.raw.len()
    }

    pub fn vector(&self, i: u32) -> &[S::Scalar] {
        self.mmap.raw.vector(i)
    }

    pub fn payload(&self, i: u32) -> Payload {
        self.mmap.raw.payload(i)
    }

    pub fn basic(
        &self,
        vector: &[S::Scalar],
        _opts: &SearchOptions,
        filter: impl Filter,
    ) -> BinaryHeap<Reverse<Element>> {
        basic(&self.mmap, vector, filter)
    }

    pub fn vbase<'a>(
        &'a self,
        vector: &'a [S::Scalar],
        _opts: &'a SearchOptions,
        filter: impl Filter + 'a,
    ) -> (Vec<Element>, Box<(dyn Iterator<Item = Element> + 'a)>) {
        vbase(&self.mmap, vector, filter)
    }
}

unsafe impl<S: G> Send for Flat<S> {}
unsafe impl<S: G> Sync for Flat<S> {}

pub struct FlatRam<S: G> {
    raw: Arc<Raw<S>>,
    quantization: Quantization<S>,
}

pub struct FlatMmap<S: G> {
    raw: Arc<Raw<S>>,
    quantization: Quantization<S>,
}

unsafe impl<S: G> Send for FlatMmap<S> {}
unsafe impl<S: G> Sync for FlatMmap<S> {}

pub fn make<S: G>(
    path: PathBuf,
    sealed: Vec<Arc<SealedSegment<S>>>,
    growing: Vec<Arc<GrowingSegment<S>>>,
    options: IndexOptions,
) -> FlatRam<S> {
    let idx_opts = options.indexing.clone().unwrap_flat();
    let raw = Arc::new(Raw::create(
        path.join("raw"),
        options.clone(),
        sealed,
        growing,
    ));
    let quantization = Quantization::create(
        path.join("quantization"),
        options.clone(),
        idx_opts.quantization,
        &raw,
    );
    FlatRam { raw, quantization }
}

pub fn save<S: G>(ram: FlatRam<S>, _: PathBuf) -> FlatMmap<S> {
    FlatMmap {
        raw: ram.raw,
        quantization: ram.quantization,
    }
}

pub fn load<S: G>(path: PathBuf, options: IndexOptions) -> FlatMmap<S> {
    let idx_opts = options.indexing.clone().unwrap_flat();
    let raw = Arc::new(Raw::open(path.join("raw"), options.clone()));
    let quantization = Quantization::open(
        path.join("quantization"),
        options.clone(),
        idx_opts.quantization,
        &raw,
    );
    FlatMmap { raw, quantization }
}

pub fn basic<S: G>(
    mmap: &FlatMmap<S>,
    vector: &[S::Scalar],
    mut filter: impl Filter,
) -> BinaryHeap<Reverse<Element>> {
    let mut result = BinaryHeap::new();
    for i in 0..mmap.raw.len() {
        let distance = mmap.quantization.distance(vector, i);
        let payload = mmap.raw.payload(i);
        if filter.check(payload) {
            result.push(Reverse(Element { distance, payload }));
        }
    }
    result
}

pub fn vbase<'a, S: G>(
    mmap: &'a FlatMmap<S>,
    vector: &'a [S::Scalar],
    mut filter: impl Filter + 'a,
) -> (Vec<Element>, Box<dyn Iterator<Item = Element> + 'a>) {
    let mut result = Vec::new();
    for i in 0..mmap.raw.len() {
        let distance = mmap.quantization.distance(vector, i);
        let payload = mmap.raw.payload(i);
        if filter.check(payload) {
            result.push(Element { distance, payload });
        }
    }
    (result, Box::new(std::iter::empty()))
}
