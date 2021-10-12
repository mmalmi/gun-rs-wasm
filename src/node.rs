use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
    RwLock
};

static COUNTER: AtomicUsize = AtomicUsize::new(1);
fn get_id() -> usize { COUNTER.fetch_add(1, Ordering::Relaxed) }

// Nodes need to be cloneable so that each instance points to the same data in the graph.
// But can we somehow wrap Node itself into Arc<RwLock<>> instead of wrapping all its properties?
// The code is not pretty with all these Arc-RwLock read/write().unwraps().
type Value<ValueType> = Arc<RwLock<Option<ValueType>>>;
type Children = Arc<RwLock<BTreeMap<String, usize>>>;
type Parents = Arc<RwLock<HashSet<(usize, String)>>>;
type Subscriptions<CallbackType> = Arc<RwLock<HashMap<usize, CallbackType>>>;
type SharedNodeStore = Arc<RwLock<HashMap<usize, Node>>>;

// TODO proper automatic tests

pub trait Callable {
    fn call(&mut self);
}

pub struct Node<ValueType: From<BTreeMap<String, usize>>, CallbackType: Callable> {
    id: usize,
    key: String,
    value: Value<ValueType>,
    children: Children,
    parents: Parents,
    on_subscriptions: Subscriptions,
    map_subscriptions: Subscriptions,
    store: SharedNodeStore
}

impl Node {
    pub fn new() -> Self {
        Self {
            id: 0,
            key: "".to_string(),
            value: Value::default(),
            children: Children::default(),
            parents: Parents::default(),
            on_subscriptions: Subscriptions::default(),
            map_subscriptions: Subscriptions::default(),
            store: SharedNodeStore::default()
        }
    }

    fn new_child(&self, key: String) -> usize {
        let mut parents = HashSet::new();
        parents.insert((self.id, key.clone()));
        let id = get_id();
        let node = Self {
            id,
            key: key.clone(),
            value: Value::default(),
            children: Children::default(),
            parents: Arc::new(RwLock::new(parents)),
            on_subscriptions: Subscriptions::default(),
            map_subscriptions: Subscriptions::default(),
            store: self.store.clone()
        };
        self.store.write().unwrap().insert(id, node);
        self.children.write().unwrap().insert(key, id);
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
            let children = self.children.read().unwrap();
            if !children.is_empty() {
                Self::_call(callback, &children.into(), key);
            }
        }
    }

    pub fn on(&mut self, callback: js_sys::Function) -> usize {
        self._call_if_value_exists(&callback, &self.key.clone());
        let subscription_id = get_id();
        self.on_subscriptions.write().unwrap().insert(subscription_id, callback);
        subscription_id
    }

    fn get_child_id(&mut self, key: String) -> usize {
        if self.value.read().unwrap().is_some() {
            self.new_child(key)
        } else {
            let existing_id = match self.children.read().unwrap().get(&key) {
                Some(node_id) => Some(*node_id),
                _ => None
            };
            match existing_id {
                Some(id) => id,
                _ => self.new_child(key)
            }
        }
    }

    pub fn get(&mut self, key: &str) -> Node {
        let id = self.get_child_id(key.to_string());
        let mut node = self.store.read().unwrap().get(&id).unwrap().clone();
        node.key = key.to_string();
        node
    }

    pub fn map(&self, callback: js_sys::Function) -> usize {
        for (key, child_id) in self.children.read().unwrap().iter() {
            if let Some(child) = self.store.read().unwrap().get(&child_id) {
                child.clone()._call_if_value_exists(&callback, key);
            }
        }
        let subscription_id = get_id();
        self.map_subscriptions.write().unwrap().insert(subscription_id, callback);
        subscription_id
    }

    pub fn put<ValueType>(&mut self, value: &ValueType) {
        // TODO handle javascript Object values
        // TODO: if "children" is replaced with "value", remove backreference from linked objects
        *self.value.write().unwrap() = Some(value.clone());
        *self.children.write().unwrap() = BTreeMap::new();
        for callback in self.on_subscriptions.read().unwrap().values() {
            Self::_call(callback, value, &self.key);
        }
        for (parent_id, key) in self.parents.read().unwrap().iter() {
            let parent = self.store.read().unwrap().get(parent_id).unwrap().clone();
            let mut parent2 = parent.clone();
            for callback in parent.clone().map_subscriptions.read().unwrap().values() {
                Self::_call(callback, value, key);
            }
            for callback in parent.on_subscriptions.read().unwrap().values() {
                parent2._call_if_value_exists(&callback, key);
            }
            *parent.value.write().unwrap() = None;
        }
    }

    fn _call<ValueType>(callback: &js_sys::Function, value: &ValueType, key: &String) {
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
        let node = gun.get("asdf");
        assert_eq!(gun.id, 0);
        assert_eq!(node.id, 1);
    }
}
