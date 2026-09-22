pub mod auth;
pub mod client;
pub mod failure;
pub mod handles;
pub mod simple;
pub mod types;

use std::sync::OnceLock;

use tokio::runtime::Runtime;

pub(crate) fn rt() -> &'static Runtime {
    static RT: OnceLock<Runtime> = OnceLock::new();
    RT.get_or_init(build_runtime)
}

fn build_runtime() -> Runtime {
    let mut builder = tokio::runtime::Builder::new_multi_thread();
    builder.enable_all();
    // Tokio 1.53 starts multi-thread workers through the blocking pool, and
    // that pool uses this size. Codex polls ConfigToml on those workers.
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    builder.thread_stack_size(kim_desktop_runtime::THREAD_STACK_SIZE_BYTES);
    builder.build().expect("tokio runtime")
}

#[cfg(all(
    test,
    any(target_os = "macos", target_os = "windows", target_os = "linux")
))]
mod tests {
    use super::build_runtime;

    #[test]
    fn worker_stack_matches_codex_budget() {
        let rt = build_runtime();
        let size = rt
            .block_on(async { tokio::spawn(async { current_stack_size() }).await })
            .expect("worker");
        assert!(
            size >= kim_desktop_runtime::THREAD_STACK_SIZE_BYTES,
            "worker stack {size} is below the Codex budget"
        );
    }

    #[cfg(target_os = "macos")]
    #[allow(unsafe_code)]
    fn current_stack_size() -> usize {
        // SAFETY: pthread_self is this thread. pthread_get_stacksize_np only
        // reads the stack size recorded for that thread.
        unsafe { libc::pthread_get_stacksize_np(libc::pthread_self()) }
    }

    #[cfg(target_os = "linux")]
    #[allow(unsafe_code)]
    fn current_stack_size() -> usize {
        // SAFETY: pthread_getattr_np initializes `attr` for this thread.
        // The stack pointer and size are written into local outs, then the
        // attribute is destroyed.
        unsafe {
            let mut attr = std::mem::MaybeUninit::<libc::pthread_attr_t>::uninit();
            assert_eq!(
                libc::pthread_getattr_np(libc::pthread_self(), attr.as_mut_ptr()),
                0,
                "pthread_getattr_np"
            );
            let mut attr = attr.assume_init();
            let mut addr = std::ptr::null_mut::<libc::c_void>();
            let mut size = 0usize;
            assert_eq!(
                libc::pthread_attr_getstack(&attr, &mut addr, &mut size),
                0,
                "pthread_attr_getstack"
            );
            assert_eq!(
                libc::pthread_attr_destroy(&mut attr),
                0,
                "pthread_attr_destroy"
            );
            size
        }
    }

    #[cfg(windows)]
    #[allow(unsafe_code)]
    fn current_stack_size() -> usize {
        unsafe extern "system" {
            fn GetCurrentThreadStackLimits(low: *mut usize, high: *mut usize);
        }
        let mut low = 0usize;
        let mut high = 0usize;
        // SAFETY: both pointers refer to local usize slots. The call writes
        // the current thread's stack bounds and does not retain them.
        unsafe { GetCurrentThreadStackLimits(&mut low, &mut high) };
        high.saturating_sub(low)
    }
}
