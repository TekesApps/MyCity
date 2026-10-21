//! Real global allocator process probe; business modules need no unsafe code.
use mycity_role_allocator::Allocator;
#[global_allocator]
static A: Allocator = Allocator::new();
#[repr(align(65536))]
struct Aligned([u8; 333]);
fn main() {
    let mode = std::env::args().nth(1).unwrap();
    let mut prefix = vec![9u64; 1024];
    let sync = std::sync::Arc::new(std::sync::Barrier::new(2));
    let other = std::sync::Arc::clone(&sync);
    // Start a monitor before activation, as the actual parent-pipe model helper does.
    let t = std::thread::spawn(move || {
        other.wait();
        for i in 0..10000 {
            let v = vec![(i % 251) as u8; 31 + i % 4096];
            std::hint::black_box(v);
        }
    });
    let activation = A.activate(8 * 1024 * 1024).unwrap();
    assert!(A.activate(8 * 1024 * 1024).is_err());
    sync.wait();
    if mode == "abort" {
        let v = vec![1u8; 16 * 1024 * 1024];
        std::hint::black_box(v);
    }
    prefix.resize(4000, 17);
    assert!(prefix[..1024].iter().all(|x| *x == 9));
    let p = Box::new(Aligned([3; 333]));
    assert_eq!((&*p as *const Aligned as usize) % 65536, 0);
    assert_eq!(p.0[332], 3);
    let mut too_big = Vec::<u8>::new();
    assert!(too_big.try_reserve_exact(16 * 1024 * 1024).is_err());
    for i in 0..10000 {
        let v = vec![(i % 251) as u8; 64 + i % 2048];
        std::hint::black_box(v);
    }
    t.join().unwrap();
    let s = A.snapshot();
    assert_eq!(s.activation, Some(activation));
    assert_eq!(s.allocation_failures, 1);
    assert!(s.current_requested_bytes < activation.grant_bytes);
    println!(
        "role_arena_process_probe=passed grant={} initialized={} bootstrap={} peak={} failures={}",
        activation.grant_bytes,
        activation.initialized_bytes,
        activation.bootstrap_charged_bytes,
        s.lifetime_peak_requested_bytes,
        s.allocation_failures
    );
}
