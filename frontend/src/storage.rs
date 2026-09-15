use std::collections::HashSet;
use web_sys::window;

const COMPLETED_KEY: &str = "btu_completed_modules_v1";
const BOOKMARKS_KEY: &str = "btu_bookmarked_modules_v1";

pub fn load_set(key: &str) -> HashSet<String> {
    if let Some(win) = window() {
        if let Ok(Some(storage)) = win.local_storage() {
            if let Ok(Some(val)) = storage.get_item(key) {
                if let Ok(set) = serde_json::from_str::<HashSet<String>>(&val) {
                    return set;
                }
            }
        }
    }
    HashSet::new()
}

pub fn save_set(key: &str, set: &HashSet<String>) {
    if let Some(win) = window() {
        if let Ok(Some(storage)) = win.local_storage() {
            if let Ok(json) = serde_json::to_string(set) {
                let _ = storage.set_item(key, &json);
            }
        }
    }
}

pub fn load_completed() -> HashSet<String> {
    load_set(COMPLETED_KEY)
}

pub fn save_completed(set: &HashSet<String>) {
    save_set(COMPLETED_KEY, set);
}

pub fn load_bookmarks() -> HashSet<String> {
    load_set(BOOKMARKS_KEY)
}

pub fn save_bookmarks(set: &HashSet<String>) {
    save_set(BOOKMARKS_KEY, set);
}
