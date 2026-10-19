#[path = "../../test_bench/src/active_object.rs"]
pub mod active_object;
#[path = "../../test_bench/src/dac.rs"]
pub mod dac;
#[path = "../../test_bench/src/screen.rs"]
pub mod screen;
#[path = "../../test_bench/src/sd.rs"]
pub mod sd;
#[path = "../../test_bench/src/shared_types.rs"]
pub mod shared_types;

// Firmware's `main.rs` restarts the chip; on the host, fail the test instead.
fn restart_on_failure(reason: &str) -> ! {
    panic!("{reason}")
}
