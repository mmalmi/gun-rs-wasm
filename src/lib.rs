use wasm_bindgen::prelude::*;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
    RwLock
};

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

static COUNTER: AtomicUsize = AtomicUsize::new(1);
fn get_id() -> usize { COUNTER.fetch_add(1, Ordering::Relaxed) }

// Nodes need to be cloneable so that each instance points to the same data in the graph.
// But can we somehow wrap Node itself into Arc<RwLock<>> instead of wrapping all its properties?
// The code is not pretty with all these Arc-RwLocks.
type ValueType = Arc<RwLock<Option<JsValue>>>;
type LinksType = Arc<RwLock<BTreeMap<String, usize>>>;
type LinkedByType = Arc<RwLock<HashSet<(usize, String)>>>;
type SubscriptionsType = Arc<RwLock<HashMap<usize, js_sys::Function>>>;
type SharedNodeStore = Arc<RwLock<HashMap<usize, Node>>>;

// TODO use &str instead of String where possible
// TODO proper automatic tests

#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct Node {
    id: usize,
    key: String,
    value: ValueType,
    links: LinksType,
    linked_by: LinkedByType,
    on_subscriptions: SubscriptionsType,
    map_subscriptions: SubscriptionsType,
    store: SharedNodeStore
}

#[wasm_bindgen]
impl Node {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            id: 0,
            key: "".to_string(),
            value: ValueType::default(),
            links: LinksType::default(),
            linked_by: LinkedByType::default(),
            on_subscriptions: SubscriptionsType::default(),
            map_subscriptions: SubscriptionsType::default(),
            store: SharedNodeStore::default()
        }
    }

    fn new_child(parent: &mut Node, key: String) -> usize {
        let mut linked_by = HashSet::new();
        linked_by.insert((parent.id, key.clone()));
        let id = get_id();
        let node = Self {
            id,
            key: key.clone(),
            value: ValueType::default(),
            links: LinksType::default(),
            linked_by: Arc::new(RwLock::new(linked_by)),
            on_subscriptions: SubscriptionsType::default(),
            map_subscriptions: SubscriptionsType::default(),
            store: parent.store.clone()
        };
        parent.store.write().unwrap().insert(id, node);
        parent.links.write().unwrap().insert(key, id);
        id
    }

    pub fn off(&mut self, subscription_id: usize) {
        self.on_subscriptions.write().unwrap().remove(&subscription_id);
        self.map_subscriptions.write().unwrap().remove(&subscription_id);
    }

    fn _call_if_value_exists(&mut self, callback: &js_sys::Function, key: &String) {
        let value = self.value.read().unwrap();
        if value.is_some() {
            Self::_call(callback, &value.as_ref().unwrap(), key);
        } else {
            let is_empty = self.links.read().unwrap().is_empty();
            if !is_empty {
                let obj = js_sys::Object::new();
                for (key, child_id) in self.links.read().unwrap().iter() {
                    let child_value: Option<JsValue> = match self.store.read().unwrap().get(&child_id) {
                        Some(child) => match &*(child.value.read().unwrap()) {
                            Some(value) => Some(value.clone()),
                            _ => None
                        },
                        _ => None
                    };
                    if let Some(value) = child_value {
                        js_sys::Reflect::set(&obj, &JsValue::from(key), &value);
                    } else { // return child Node object
                        js_sys::Reflect::set(
                            &obj,
                            &JsValue::from(key),
                            &self.store.read().unwrap().get(&child_id).unwrap().clone().into()
                        );
                    }
                }
                Self::_call(callback, &obj, key);
            }
        }
    }

    pub fn on(&mut self, callback: js_sys::Function) -> usize {
        self._call_if_value_exists(&callback, &self.key.clone()); // TODO return the actual key
        let subscription_id = get_id();
        self.on_subscriptions.write().unwrap().insert(subscription_id, callback);
        subscription_id
    }

    fn get_child_id(&mut self, key: String) -> usize {
        if self.value.read().unwrap().is_some() {
            Node::new_child(self, key)
            // TODO: nullify self.value
        } else {
            let existing_id = match self.links.read().unwrap().get(&key) {
                Some(node_id) => Some(*node_id),
                _ => None
            };
            match existing_id {
                Some(id) => id,
                _ => Node::new_child(&mut self.clone(), key)
            }
        }
    }

    pub fn get(&mut self, key: String) -> Node {
        let id = self.get_child_id(key.clone());
        let mut node = self.store.read().unwrap().get(&id).unwrap().clone();
        node.key = key;
        node
    }

    pub fn map(&self, callback: js_sys::Function) -> usize {
        for (key, child_id) in self.links.read().unwrap().iter() {
            if let Some(child) = self.store.read().unwrap().get(&child_id) {
                child.clone()._call_if_value_exists(&callback, key);
            }
        }
        let subscription_id = get_id();
        self.map_subscriptions.write().unwrap().insert(subscription_id, callback);
        subscription_id
    }

    pub fn put(&mut self, value: &JsValue) {
        // TODO handle javascript Object values
        // TODO: if "links" is replaced with "value", remove backreference from linked objects
        *(self.value.write().unwrap()) = Some(value.clone());
        *(self.links.write().unwrap()) = BTreeMap::new();
        for callback in self.on_subscriptions.read().unwrap().values() {
            Self::_call(callback, value, &self.key);
        }
        for (parent_id, key) in self.linked_by.read().unwrap().iter() {
            let parent = self.store.read().unwrap().get(parent_id).unwrap().clone();
            let mut parent2 = parent.clone();
            for callback in parent.clone().map_subscriptions.read().unwrap().values() {
                Self::_call(callback, value, key);
            }
            for callback in parent.on_subscriptions.read().unwrap().values() {
                parent2._call_if_value_exists(&callback, key);
            }
        }
    }

    fn _call(callback: &js_sys::Function, value: &JsValue, key: &String) {
        let _ = callback.call2(&JsValue::null(), value, &JsValue::from(key)); // can the function go out of scope? remove sub on Err
    }
}

#[cfg(test)]
mod tests {
    // TODO proper test
    // TODO benchmark
    #[test]
    fn it_works() {
        let mut gun = crate::Node::new();
        let node = gun.get("asdf".to_string());
        assert_eq!(gun.id, 0);
        assert_eq!(node.id, 1);
    }
}
