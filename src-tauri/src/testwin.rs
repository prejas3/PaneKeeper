// CCS/1 M:live-test-windows | PURPOSE:test-owned windows (examples/testwin.rs) for the ignored live tests
// RULE:unique title per window (pid + time + tag), so parallel tests never match each other's windows
// RULE:Drop kills every child, so a failed assert still closes them

use crate::windows::{self, Rect, WindowSnapshot};
use std::{
    path::PathBuf,
    process::{Child, Command},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub struct TestWindows {
    children: Vec<Child>,
    pub pids: Vec<u32>,
}

/// `cargo test` builds the examples next to the test binary: target/<profile>/examples/testwin.exe.
fn exe() -> PathBuf {
    let deps = std::env::current_exe().unwrap();
    deps.parent().unwrap().parent().unwrap().join("examples").join("testwin.exe")
}

pub fn wait_for(mut ready: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        if ready() {
            return true;
        }
        thread::sleep(Duration::from_millis(50));
    }
    false
}

impl TestWindows {
    pub fn open(tag: &str, rects: &[Rect]) -> Self {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let mut this = Self { children: Vec::new(), pids: Vec::new() };
        for (i, r) in rects.iter().enumerate() {
            let title = format!("wk-{tag}-{}-{stamp}-{i}", std::process::id());
            let child = Command::new(exe())
                .args(["window", &title])
                .args([r.left, r.top, r.right, r.bottom].map(|n| n.to_string()))
                .spawn()
                .expect("testwin.exe (built by cargo test as an example)");
            this.pids.push(child.id());
            this.children.push(child);
        }
        assert!(wait_for(|| this.capture().len() == rects.len()), "test windows did not appear");
        this
    }

    pub fn capture(&self) -> Vec<WindowSnapshot> {
        unsafe { windows::test_capture_processes(&self.pids) }
    }

    /// Every saved window sits within 2 px of where it was saved.
    pub fn back_in_place(&self, saved: &[WindowSnapshot]) -> bool {
        wait_for(|| {
            let current = self.capture();
            saved.iter().all(|expected| {
                current.iter().any(|actual| {
                    let (a, e) = (actual.normal_rect, expected.normal_rect);
                    actual.title == expected.title
                        && (a.left - e.left).abs() <= 2
                        && (a.top - e.top).abs() <= 2
                        && (a.right - e.right).abs() <= 2
                        && (a.bottom - e.bottom).abs() <= 2
                })
            })
        })
    }
}

impl Drop for TestWindows {
    fn drop(&mut self) {
        for child in &mut self.children {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
