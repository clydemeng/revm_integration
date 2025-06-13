// Define the signature of the Go callback function in Rust
type GoCallback = extern "C" fn(i32) -> i32;

/// # Safety
/// This function is unsafe because it dereferences a raw pointer passed from C.
#[no_mangle]
pub extern "C" fn process_data_in_rust(go_callback: GoCallback, value: i32) -> i32 {
    println!("[Rust] Received value: {}", value);
    println!("[Rust] Calling Go callback...");

    // Invoke the Go function pointer
    let result = go_callback(value);

    println!("[Rust] Received result from Go callback: {}", result);
    result + 1
}

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
