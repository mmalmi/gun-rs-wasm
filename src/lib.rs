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

// Nodes need to be clonable so that each instance points to the same data
type ValueType = Arc<RwLock<Option<JsValue>>>;
type LinksType = Arc<RwLock<BTreeMap<String, usize>>>;
type LinkedByType = Arc<RwLock<HashSet<usize>>>;
type SubscriptionsType = Arc<RwLock<HashMap<usize, js_sys::Function>>>;
type SharedNodeStore = Arc<RwLock<HashMap<usize, Node>>>;

#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct Node {
    id: usize,
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
            value: ValueType::default(),
            links: LinksType::default(),
            linked_by: LinkedByType::default(),
            on_subscriptions: SubscriptionsType::default(),
            map_subscriptions: SubscriptionsType::default(),
            store: SharedNodeStore::default()
        }
    }

    fn new_child(parent: &mut Node, path: String) -> usize {
        let mut linked_by = HashSet::new();
        linked_by.insert(parent.id);
        let id = get_id();
        let node = Self {
            id,
            value: ValueType::default(),
            links: LinksType::default(),
            linked_by: Arc::new(RwLock::new(linked_by)),
            on_subscriptions: SubscriptionsType::default(),
            map_subscriptions: SubscriptionsType::default(),
            store: parent.store.clone()
        };
        parent.store.write().unwrap().insert(id, node);
        parent.links.write().unwrap().insert(path, id);
        id
    }

    pub fn off(&mut self, subscription_id: usize) {
        self.on_subscriptions.write().unwrap().remove(&subscription_id);
        self.map_subscriptions.write().unwrap().remove(&subscription_id);
    }

    fn _call_if_value_exists(&mut self, callback: &js_sys::Function) {
        let value = self.value.read().unwrap();
        if value.is_some() {
            Self::_call(callback, &value.as_ref().unwrap());
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
                    } else {
                        js_sys::Reflect::set(&obj, &JsValue::from(key), &JsValue::NULL); // TODO return Node self.store.read().unwrap().get(&id).unwrap().clone()
                    }
                }
                Self::_call(callback, &obj);
            }
        }
    }

    pub fn on(&mut self, callback: js_sys::Function) -> usize {
        self._call_if_value_exists(&callback);
        let subscription_id = get_id();
        self.on_subscriptions.write().unwrap().insert(subscription_id, callback);
        subscription_id
    }

    fn get_child_id(&mut self, path: String) -> usize {
        if self.value.read().unwrap().is_some() {
            Node::new_child(self, path)
            // TODO: nullify self.value
        } else {
            let existing_id = match self.links.read().unwrap().get(&path) {
                Some(node_id) => Some(*node_id),
                _ => None
            };
            match existing_id {
                Some(id) => id,
                _ => Node::new_child(&mut self.clone(), path)
            }
        }
    }

    pub fn get(&mut self, path: String) -> Node {
        let id = self.get_child_id(path);
        self.store.read().unwrap().get(&id).unwrap().clone()
    }

    pub fn map(&self, callback: js_sys::Function) -> usize {
        for (key, child_id) in self.links.read().unwrap().iter() {
            if let Some(child) = self.store.read().unwrap().get(&child_id) {
                child.clone()._call_if_value_exists(&callback);
            }
        }
        let subscription_id = get_id();
        self.map_subscriptions.write().unwrap().insert(subscription_id, callback);
        subscription_id
    }

    pub fn put(&mut self, value: &JsValue) {
        *(self.value.write().unwrap()) = Some(value.clone());
        *(self.links.write().unwrap()) = BTreeMap::new();
        for callback in self.on_subscriptions.read().unwrap().values() {
            Self::_call(callback, value);
        }
        for parent_id in self.linked_by.read().unwrap().iter() {
            let parent = self.store.read().unwrap().get(parent_id).unwrap().clone();
            for callback in parent.map_subscriptions.read().unwrap().values() {
                Self::_call(callback, value);
            }
        }
    }

    fn _call(callback: &js_sys::Function, value: &JsValue) {
        let _ = callback.call1(&JsValue::null(), value); // can the function go out of scope? remove sub on Err
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let mut gun = crate::Node::new();
        let node = gun.get("asdf".to_string());
        assert_eq!(gun.id, 0);
        assert_eq!(node.id, 1);
    }
}
