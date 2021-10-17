static CHARACTERS: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
static CHARACTERS_LENGTH: f64 = CHARACTERS.len() as f64;

pub fn random_string(len: usize) -> String {
    let mut result = "".to_string();
    for _ in 0..len {
        let random_index = (js_sys::Math::random() * CHARACTERS_LENGTH).floor() as usize;
        result.push(CHARACTERS.chars().nth(random_index).unwrap());
    }
    result
}

pub fn set_panic_hook() {
    // When the `console_error_panic_hook` feature is enabled, we can call the
    // `set_panic_hook` function at least once during initialization, and then
    // we will get better error messages if our code ever panics.
    //
    // For more details see
    // https://github.com/rustwasm/console_error_panic_hook#readme
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}
