use std::sync::OnceLock;

fn verbose_setting() -> bool {
    match std::env::var("VERBOSE") {
        Ok(value) => {
            let value = value.to_ascii_lowercase();

            if value == "0" || value == "false" || value == "no" || value == "off" {
                return false;
            }

            value == "1" || value == "true" || value == "yes" || value == "on"
        }
        Err(_) => cfg!(debug_assertions),
    }
}

pub fn verbose_enabled() -> bool {
    static VERBOSE: OnceLock<bool> = OnceLock::new();
    *VERBOSE.get_or_init(verbose_setting)
}

pub fn vprintln(args: std::fmt::Arguments<'_>) {
    if verbose_enabled() {
        println!("{}", args);
    }
}
