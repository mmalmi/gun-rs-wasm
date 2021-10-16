use wasm_bindgen::prelude::*;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
    RwLock
};
use wasm_bindgen::JsCast;
use web_sys::{ErrorEvent, MessageEvent, WebSocket};
use js_sys::{JSON, Reflect, Object as JsObject, Array as JsArray};
use rand::{thread_rng, Rng};
use rand::distributions::Alphanumeric;
use serde_json::{json, Value as SerdeJsonValue};


macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

static COUNTER: AtomicUsize = AtomicUsize::new(1);
fn get_id() -> usize { COUNTER.fetch_add(1, Ordering::Relaxed) }

// Nodes need to be cloneable so that each instance points to the same data in the graph.
// But can we somehow wrap Node itself into Arc<RwLock<>> instead of wrapping all its properties?
// The code is not pretty with all these Arc-RwLock read/write().unwraps().
type Value = Arc<RwLock<Option<JsValue>>>;
type Children = Arc<RwLock<BTreeMap<String, usize>>>;
type Parents = Arc<RwLock<HashSet<(usize, String)>>>;
type Subscriptions = Arc<RwLock<HashMap<usize, js_sys::Function>>>;
type SharedNodeStore = Arc<RwLock<HashMap<usize, Node>>>;
type SharedWebSocket = Arc<RwLock<Option<WebSocket>>>;

// TODO use &str instead of String where possible
// TODO proper automatic tests
// TODO generic version for non-wasm usage
// TODO break into submodules
// TODO websocket
// TODO persist data by saving root node to indexedDB as serialized by serde?

#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct Node {
    id: usize,
    updated_at: Arc<RwLock<i64>>, // TODO option?
    key: String,
    path: Vec<String>,
    value: Value,
    children: Children,
    parents: Parents,
    on_subscriptions: Subscriptions,
    map_subscriptions: Subscriptions,
    store: SharedNodeStore,
    websocket: SharedWebSocket
}

fn random_string(len: usize) -> String {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(len)
        .map(char::from)
        .collect()
}

#[wasm_bindgen]
impl Node {
    #[wasm_bindgen(constructor)]
    pub fn new(options: &JsValue) -> Self {
        let node = Self {
            id: 0,
            updated_at: Arc::new(RwLock::new(0)),
            key: "".to_string(),
            path: Vec::new(),
            value: Value::default(),
            children: Children::default(),
            parents: Parents::default(),
            on_subscriptions: Subscriptions::default(),
            map_subscriptions: Subscriptions::default(),
            store: SharedNodeStore::default(),
            websocket: SharedWebSocket::default()
        };
        console_error_panic_hook::set_once();
        node.handle_options(options);
        node
    }

    fn handle_options(&self, options: &JsValue) {
        let mut peers = Vec::new();
        if let Some(peer) = options.as_string() {
            peers.push(peer);
        }
        if let Some(object) = JsObject::try_from(options) { // horrible. can JsValue processing be improved?
            if let Ok(val) = Reflect::get(object, &JsValue::from("peers")) {
                if JsArray::is_array(&val) {
                    for peer in JsArray::from(&val).iter() {
                        if let Some(peer) = peer.as_string() {
                            peers.push(peer.to_string());
                        }
                    }
                }
            }
        }

        for peer in peers {
            let _ = self.start_websocket(peer);
        }
    }

    fn start_websocket(&self, url: String) -> Result<(), JsValue> {
        // Connect to an echo server
        let ws = WebSocket::new(&url)?;
        // For small binary messages, like CBOR, Arraybuffer is more efficient than Blob handling
        ws.set_binary_type(web_sys::BinaryType::Arraybuffer);
        // create callback

        let mut cloned_self = self.clone();
        let onmessage_callback = Closure::wrap(Box::new(move |e: MessageEvent| {
            if let Ok(txt) = e.data().dyn_into::<js_sys::JsString>() {
                console_log!("received: {}", txt);
                if let Ok(json) = serde_json::from_str::<SerdeJsonValue>(&String::from(txt.clone())) {
                    cloned_self.incoming_message(&json, false);
                }
            } else {
                console_log!("message event, received Unknown: {:?}", e.data());
            }
        }) as Box<dyn FnMut(MessageEvent)>);
        // set message event handler on WebSocket
        ws.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
        // forget the callback to keep it alive
        onmessage_callback.forget();

        let onerror_callback = Closure::wrap(Box::new(move |e: ErrorEvent| {
            console_log!("websocket error: {:?}", e);
        }) as Box<dyn FnMut(ErrorEvent)>);
        ws.set_onerror(Some(onerror_callback.as_ref().unchecked_ref()));
        onerror_callback.forget();

        let cloned_ws = ws.clone();

        let cloned_ws_pointer = self.websocket.clone();
        let onopen_callback = Closure::wrap(Box::new(move |_| {
            console_log!("socket opened");
            let msg_id = random_string(8);
            let peer_id = random_string(8);
            let m = format!("{{\"#\":\"{}\",\"dam\":\"hi\",\"pid\":\"{}\"}}", msg_id, peer_id);
            match cloned_ws.send_with_str(&m) {
                Ok(_) => console_log!("sent: {}", m),
                Err(err) => console_log!("error sending hi-message: {:?}", err),
            }
            cloned_ws_pointer.write().unwrap().insert(cloned_ws.clone());
        }) as Box<dyn FnMut(JsValue)>);
        ws.set_onopen(Some(onopen_callback.as_ref().unchecked_ref()));
        onopen_callback.forget();
        Ok(())
    }

    fn new_child(&self, key: String) -> usize {
        assert!(key.len() > 0, "Key length must be greater than zero");
        let mut parents = HashSet::new();
        parents.insert((self.id, key.clone()));
        let mut path = self.path.clone();
        if self.key.len() > 0 {
            path.push(self.key.clone());
        }
        let id = get_id();
        let node = Self {
            id,
            updated_at: Arc::new(RwLock::new(0)),
            key: key.clone(),
            path,
            value: Value::default(),
            children: Children::default(),
            parents: Arc::new(RwLock::new(parents)),
            on_subscriptions: Subscriptions::default(),
            map_subscriptions: Subscriptions::default(),
            store: self.store.clone(),
            websocket: self.websocket.clone()
        };
        self.store.write().unwrap().insert(id, node);
        self.children.write().unwrap().insert(key, id);
        id
    }

    pub fn off(&mut self, subscription_id: usize) {
        self.on_subscriptions.write().unwrap().remove(&subscription_id);
        self.map_subscriptions.write().unwrap().remove(&subscription_id);
    }

    pub fn on(&mut self, callback: js_sys::Function) -> usize {
        self._call_if_value_exists(&callback, &self.key.clone());
        let subscription_id = get_id();
        self.on_subscriptions.write().unwrap().insert(subscription_id, callback);

        if let Some(ws) = &*self.websocket.read().unwrap() {
            let m = self.create_get_msg();
            match ws.send_with_str(&m) {
                Ok(_) => console_log!("sent: {}", m),
                Err(err) => console_log!("error sending message: {:?}", err),
            }
        }

        subscription_id
    }

    pub fn get(&mut self, key: &str) -> Node {
        let id = self.get_child_id(key.to_string());
        let mut node = self.store.read().unwrap().get(&id).unwrap().clone();
        node.key = key.to_string();
        node
    }

    pub fn map(&self, callback: js_sys::Function) -> usize {
        for (key, child_id) in self.children.read().unwrap().iter() { // TODO can be faster with rayon multithreading?
            if let Some(child) = self.store.read().unwrap().get(&child_id) {
                child.clone()._call_if_value_exists(&callback, key);
            }
        }
        let subscription_id = get_id();
        self.map_subscriptions.write().unwrap().insert(subscription_id, callback);
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

    fn create_get_msg(&self) -> String {
        let msg_id = random_string(8);
        let key = self.key.clone();
        if self.path.len() > 0 {
            let path = self.path.join("/");
            json!({
                "get": {
                    "#": path,
                    ".": key
                },
                "#": msg_id
            }).to_string()
        } else {
            json!({
                "get": {
                    "#": key
                },
                "#": msg_id
            }).to_string()
        }
    }

    fn create_put_msg(&self, value: &JsValue, updated_at: i64) -> String {
        let value: String = JSON::stringify(value).unwrap().into(); // TODO non-strings shouldn't be stringified
        let msg_id = random_string(8);
        let full_path = &self.path.join("/");
        let key = &self.key.clone();
        let mut json = json!({
            "put": {
                full_path: {
                    "_": {
                        "#": full_path,
                        ">": {
                            key: updated_at
                        }
                    },
                    key: &value
                }
            },
            "#": msg_id,
        });

        let puts = &mut json["put"];
        // if it's a nested node, put its parents also
        for (i, node_name) in self.path.iter().enumerate().nth(1) {
            let path = self.path[..i].join("/");
            let path_obj = json!({
                "_": {
                    "#": path,
                    ">": {
                        node_name: updated_at
                    }
                },
                node_name: {
                    "#": self.path[..(i+1)].join("/")
                }
            });
            puts[path] = path_obj;
        }
        json.to_string()
    }

    fn incoming_message(&mut self, msg: &SerdeJsonValue, is_from_array: bool) {
        if let Some(array) = msg.as_array() {
            if is_from_array { return; } // don't allow array inside array
            for msg in array.iter() {
                self.incoming_message(msg, true);
            }
            return;
        }
        if let Some(obj) = msg.as_object() {
            if let Some(put) = obj.get("put") {
                if let Some(obj) = put.as_object() {
                    self.incoming_put(obj);
                }
            }
            if let Some(get) = obj.get("get") {
                if let Some(obj) = get.as_object() {
                    self.incoming_get(obj);
                }
            }
        }
    }

    fn incoming_put(&mut self, put: &serde_json::Map<String, SerdeJsonValue>) {
        for (updated_key, update_data) in put.iter() {
            let mut node = self.get(updated_key);
            for node_name in updated_key.split("/").nth(1) {
                node = node.get(node_name);
            }
            if let Some(updated_at_times) = update_data["_"][">"].as_object() {
                for (child_key, incoming_val_updated_at) in updated_at_times.iter() {
                    let incoming_val_updated_at = incoming_val_updated_at.as_i64().unwrap();
                    let mut child = node.get(child_key);
                    if *child.updated_at.read().unwrap() < incoming_val_updated_at {
                        // TODO if incoming_val_updated_at > current_time { defer_operation() }
                        if let Some(new_value) = update_data.get(child_key) {
                            let new_value = JsValue::from_serde(new_value).unwrap();
                            child.put_local(&new_value, incoming_val_updated_at);
                        }
                    } // TODO else append to history
                }
            }
        }
    }

    fn _children_to_js_value(&self, children: &BTreeMap<String, usize>) -> JsValue {
        let obj = JsObject::new();
        for (key, child_id) in children.iter() { // TODO faster with rayon?
            let child_value: Option<JsValue> = match self.store.read().unwrap().get(&child_id) {
                Some(child) => match &*(child.value.read().unwrap()) {
                    Some(value) => Some(value.clone()),
                    _ => None
                },
                _ => None
            };
            if let Some(value) = child_value {
                let _ = Reflect::set(&obj, &JsValue::from(key), &value);
            } else { // return child Node object
                let _ = Reflect::set(
                    &obj,
                    &JsValue::from(key),
                    &self.store.read().unwrap().get(&child_id).unwrap().clone().into()
                );
            }
        }
        obj.into()
    }

    fn _call_if_value_exists(&mut self, callback: &js_sys::Function, key: &String) {
        if let Some(value) = self.get_js_value() {
            Self::_call(callback, &value.as_ref(), key);
        }
    }

    fn ws_send(&self, msg: &String) {
        if let Some(ws) = &*self.websocket.read().unwrap() {
            match ws.send_with_str(&msg) {
                Ok(_) => console_log!("sent: {}", msg),
                Err(err) => console_log!("error sending message: {:?}", err),
            }
        }
    }

    fn get_js_value(&self) -> Option<JsValue> {
        let value = self.value.read().unwrap();
        if value.is_some() {
            value.clone()
        } else {
            let children = self.children.read().unwrap();
            if !children.is_empty() {
                let obj = self._children_to_js_value(&children);
                return Some(obj)
            }
            None
        }
    }

    fn send_get_response_if_have(&self) {
        if let Some(value) = self.get_js_value() {
            let msg_id = random_string(8);
            let full_path = &self.path.join("/");
            let key = &self.key.clone();
            let value: String = JSON::stringify(&value).unwrap().into(); // TODO non-strings shouldn't be stringified
            let json = json!({
                "put": {
                    full_path: {
                        "_": {
                            "#": full_path,
                            ">": {
                                key: &*self.updated_at.read().unwrap()
                            }
                        },
                        key: &value
                    }
                },
                "#": msg_id,
            }).to_string();
            self.ws_send(&json);
        }
    }

    fn incoming_get(&mut self, get: &serde_json::Map<String, SerdeJsonValue>) {
        console_log!("incoming get {:?}", get);
        if let Some(path) = get.get("#") {
            if let Some(path) = path.as_str() {
                if let Some(key) = get.get(".") {
                    if let Some(key) = key.as_str() {
                        let mut split = path.split("/");
                        let mut node = self.get(split.nth(0).unwrap());
                        for node_name in split.nth(0) {
                            node = node.get(node_name); // TODO get only existing nodes in order to not spam our graph with empties
                        }
                        node = node.get(key);
                        node.send_get_response_if_have();
                    }
                } else {
                    self.get(path).send_get_response_if_have();
                }
            }
        }
    }

    pub fn put(&mut self, value: &JsValue) {
        let time = js_sys::Date::now() as i64;
        self.put_local(value, time);
        if let Some(ws) = &*self.websocket.read().unwrap() {
            let m = self.create_put_msg(&value, time);
            match ws.send_with_str(&m) {
                Ok(_) => console_log!("sent: {}", m),
                Err(err) => console_log!("error sending message: {:?}", err),
            }
        }
    }

    fn put_local(&mut self, value: &JsValue, time: i64) {
        // root.get(soul).get(key).put(jsvalue)
        // TODO handle javascript Object values
        // TODO: if "children" is replaced with "value", remove backreference from linked objects
        *self.updated_at.write().unwrap() = time;
        *self.value.write().unwrap() = Some(value.clone());
        *self.children.write().unwrap() = BTreeMap::new();
        for callback in self.on_subscriptions.read().unwrap().values() { // rayon?
            Self::_call(callback, value, &self.key);
        }
        for (parent_id, key) in self.parents.read().unwrap().iter() { // rayon?
            if let Some(parent) = self.store.read().unwrap().get(parent_id) {
                let mut parent_clone = parent.clone();
                for callback in parent.clone().map_subscriptions.read().unwrap().values() {
                    Self::_call(callback, value, key);
                }
                for callback in parent.on_subscriptions.read().unwrap().values() {
                    parent_clone._call_if_value_exists(&callback, key);
                }
                *parent.value.write().unwrap() = None;
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
        let node = gun.get("asdf");
        assert_eq!(gun.id, 0);
        assert_eq!(node.id, 1);
    }
}
