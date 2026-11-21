use super::*;
use core::{ptr, slice};

#[test]
fn zeroing_and_resize_preserve_layout_at_native_boundaries() {
    let limit = mi::MI_SMALL_SIZE_MAX;
    for align in [1, 2, 8, 16, 64, 4096, 65536] {
        for size in [
            1,
            7,
            8,
            17,
            limit - 1,
            limit,
            limit + 1,
            8193,
            65535,
            65536,
            65537,
        ] {
            let layout = Layout::from_size_align(size, align).unwrap();
            let grown_size = size + limit;
            let grown_layout = Layout::from_size_align(grown_size, align).unwrap();
            // SAFETY: Layouts are nonempty and valid. We only access initialized
            // bytes inside live allocations. Each successful resize transfers
            // ownership and we free the final pointer with its current layout.
            unsafe {
                let dirty = DesktopMiMalloc.alloc(layout);
                assert!(!dirty.is_null());
                assert_eq!(dirty.addr() % align, 0);
                ptr::write_bytes(dirty, 0xff, size);
                DesktopMiMalloc.dealloc(dirty, layout);

                let block = DesktopMiMalloc.alloc_zeroed(layout);
                assert!(!block.is_null());
                assert_eq!(block.addr() % align, 0);
                assert!(slice::from_raw_parts(block, size).iter().all(|&b| b == 0));
                ptr::write_bytes(block, 0xa5, size);

                let grown = DesktopMiMalloc.realloc(block, layout, grown_size);
                assert!(!grown.is_null());
                assert_eq!(grown.addr() % align, 0);
                assert!(
                    slice::from_raw_parts(grown, size)
                        .iter()
                        .all(|&b| b == 0xa5)
                );

                let small = DesktopMiMalloc.realloc(grown, grown_layout, 1);
                assert!(!small.is_null());
                assert_eq!(small.addr() % align, 0);
                assert_eq!(small.read(), 0xa5);
                DesktopMiMalloc.dealloc(small, Layout::from_size_align(1, align).unwrap());
            }
        }
    }
}
