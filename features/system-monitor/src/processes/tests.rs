use super::fixture::{ProcTree, own_uid};
use super::*;

fn entry(id: &str) -> Option<DesktopEntry> {
    match id {
        "org.example.browser" => Some(DesktopEntry {
            name: "Example Browser".into(),
            icon: Some("example-browser".into()),
        }),
        _ => None,
    }
}

fn sampler(tree: &ProcTree) -> ProcessSampler {
    let mut sampler = ProcessSampler::open_in(tree.root(), &tree.root().join("no-drm"));
    sampler.resolve = entry;
    sampler
}

/// A browser of three processes in one Plasma unit, a terminal surface with a shell and a
/// build under it, and a process of another session outside any unit.
fn desktop() -> ProcTree {
    let tree = ProcTree::new();
    let uid = own_uid();
    for (pid, command, parent) in
        [(100, "browser", 1), (101, "browser-gpu", 100), (102, "renderer", 100)]
    {
        tree.process(pid, command, parent, 0, u64::from(pid));
        tree.cgroup(pid, uid, "app.slice/app-org.example.browser@b1.service");
    }
    tree.process(200, "zsh", 1, 0, 200);
    tree.cgroup(200, uid, "app.slice/app-terminal-surface-7.scope");
    tree.process(201, "toolchain", 200, 0, 201);
    tree.cgroup(201, uid, "app.slice/app-terminal-surface-7.scope");
    tree.process(300, "lonely", 1, 0, 300);
    tree
}

fn ready<T>(reading: Reading<T>) -> T {
    match reading {
        Reading::Ready(value) => value,
        Reading::Priming => panic!("expected a reading"),
    }
}

fn close(left: f32, right: f32) -> bool {
    (left - right).abs() < 1e-4
}

#[test]
fn the_first_cpu_request_primes_and_the_second_reconciles_with_the_machine() {
    let tree = desktop();
    tree.machine(1000, 1000);
    let mut sampler = sampler(&tree);
    assert_eq!(sampler.cpu(10), Reading::Priming);

    // 1000 ticks pass on the machine, 600 of them busy: the browser spends 300 across its
    // processes, the build 100, and the rest is somebody else's.
    tree.machine(1600, 1400);
    tree.process(100, "browser", 1, 100, 100);
    tree.process(102, "renderer", 100, 200, 102);
    tree.process(201, "toolchain", 200, 100, 201);
    let fold = ready(sampler.cpu(10));
    assert!(close(fold.machine_load, 0.6));
    assert_eq!(fold.threads, 2);
    assert_eq!(fold.applications.len(), 2, "idle applications are not listed at zero");

    let browser = &fold.applications[0];
    assert_eq!(browser.name, "Example Browser");
    assert_eq!(browser.icon.as_deref(), Some("example-browser"));
    assert!(close(browser.value.of_machine, 0.3));
    assert!(close(browser.value.of_one_core, 0.6));
    assert_eq!(browser.members.len(), 3, "the idle helper is still a member");

    let terminal = &fold.applications[1];
    assert_eq!(terminal.key, ApplicationKey::Unit("app-terminal-surface-7.scope".into()));
    assert_eq!(terminal.name, "zsh", "named after the process the rest descend from");
    assert!(close(terminal.value.of_machine, 0.1));
    assert!(close(fold.elsewhere, 0.2));
}

#[test]
fn processes_cannot_add_up_to_more_than_the_machine_did() {
    let tree = desktop();
    tree.machine(0, 1000);
    let mut sampler = sampler(&tree);
    sampler.cpu(10);
    tree.machine(100, 1900);
    tree.process(100, "browser", 1, 300, 100);
    let fold = ready(sampler.cpu(10));
    assert!(close(fold.applications[0].value.of_machine, 0.1));
    assert!(close(fold.elsewhere, 0.0));
}

#[test]
fn a_stale_baseline_primes_again() {
    let tree = desktop();
    let mut sampler = sampler(&tree);
    sampler.cpu(10);
    sampler.cpu.as_mut().unwrap().at -= BASELINE_LIMIT + Duration::from_secs(1);
    assert_eq!(sampler.cpu(10), Reading::Priming);
    sampler.disk(10);
    sampler.disk.as_mut().unwrap().at -= BASELINE_LIMIT + Duration::from_secs(1);
    assert_eq!(sampler.disk(10), Reading::Priming);
}

#[test]
fn a_reused_pid_starts_from_nothing_rather_than_from_the_old_process() {
    let tree = desktop();
    tree.machine(0, 0);
    tree.process(300, "lonely", 1, 5000, 300);
    let mut sampler = sampler(&tree);
    sampler.cpu(10);
    tree.machine(500, 500);
    tree.process(300, "newcomer", 1, 10, 999);
    let fold = ready(sampler.cpu(10));
    let newcomer = fold.applications.iter().find(|app| app.name == "newcomer").unwrap();
    assert!(close(newcomer.value.of_machine, 0.01));
    assert_eq!(newcomer.members[0].key, ProcessKey { pid: 300, started: 999 });
}

#[test]
fn another_users_processes_are_not_walked() {
    let tree = desktop();
    let mut sampler = sampler(&tree);
    sampler.uid = own_uid() + 1;
    assert!(sampler.memory(10).applications.is_empty());
}

#[test]
fn memory_prefers_the_proportional_figure_and_says_which_it_used() {
    let tree = desktop();
    let page = page_size();
    let mib = 1024 * 1024 / page;
    for pid in [100, 101, 102] {
        tree.pages(pid, 100 * mib, 10 * mib);
    }
    tree.pss_kib(100, 40 * 1024);
    tree.pss_kib(101, 20 * 1024);
    tree.pss_kib(102, 20 * 1024);
    tree.pages(200, 5 * mib, 0);
    tree.pss_kib(200, 3 * 1024);
    tree.pages(201, 30 * mib, 0);
    tree.pages(300, 0, 0);

    let fold = sampler(&tree).memory(10);
    let browser = &fold.applications[0];
    assert_eq!(browser.name, "Example Browser");
    assert_eq!(
        browser.value,
        MemoryUse { bytes: 80 * 1024 * 1024, basis: MemoryBasis::Proportional }
    );
    let terminal = &fold.applications[1];
    assert_eq!(terminal.value.basis, MemoryBasis::Mixed, "the build has no smaps_rollup");
    assert_eq!(terminal.value.bytes, 33 * 1024 * 1024);
    assert_eq!(fold.applications.len(), 2);
    assert_eq!(fold.quiet, 1, "an empty process is under the floor");
}

#[test]
fn memory_reads_smaps_only_for_the_head_of_the_list() {
    let tree = ProcTree::new();
    let mib = 1024 * 1024 / page_size();
    for (at, pid) in (600..610).enumerate() {
        tree.process(pid, &format!("tool{at}"), 1, 0, u64::from(pid));
        tree.pages(pid, (20 + at as u64) * mib, 0);
        tree.pss_kib(pid, (19 + at as u64) * 1024);
    }
    let mut sampler = sampler(&tree);
    let fold = sampler.memory(2);
    assert_eq!(fold.applications.len(), 2);
    assert_eq!(fold.quiet, 8);
    assert_eq!(sampler.pss.len(), 5, "the two listed, half as many again, and two more");
    assert_eq!(fold.applications[0].value.basis, MemoryBasis::Proportional);
}

#[test]
fn disk_rates_keep_reading_and_writing_apart() {
    let tree = desktop();
    tree.io(201, 0, 0);
    tree.io(100, 1000, 0);
    let mut sampler = sampler(&tree);
    assert_eq!(sampler.disk(10), Reading::Priming);
    tree.io(201, 4 * 1024 * 1024, 2 * 1024 * 1024);
    tree.io(100, 1000, 0);
    sampler.disk.as_mut().unwrap().at -= Duration::from_secs(2);
    let fold = ready(sampler.disk(10));
    assert_eq!(fold.applications.len(), 1);
    let rate = fold.applications[0].value;
    let near = |value: u64, expected: u64| value.abs_diff(expected) < expected / 50;
    assert!(near(rate.read_per_second, 2 * 1024 * 1024), "{rate:?}");
    assert!(near(rate.written_per_second, 1024 * 1024), "{rate:?}");
}

/// Two processes share one amdgpu client through an inherited descriptor, a third holds an
/// NVIDIA descriptor that carries no client statistics.
fn amd_desktop(busy_ns: u64) -> ProcTree {
    let tree = desktop();
    tree.file("drm/card0/device/uevent", "DRIVER=amdgpu\nPCI_SLOT_NAME=0000:0c:00.0\n");
    let driver = tree.root().join("drm/card0/device/driver");
    if !driver.exists() {
        std::os::unix::fs::symlink("../../bus/pci/drivers/amdgpu", driver).unwrap();
    }
    let info = format!(
        "drm-driver:\tamdgpu\ndrm-client-id:\t9\ndrm-pdev:\t0000:0c:00.0\n\
         drm-engine-gfx:\t{busy_ns} ns\ndrm-total-vram:\t64 MiB\n"
    );
    tree.descriptor(100, 7, "/dev/dri/renderD129", &info);
    tree.descriptor(102, 7, "/dev/dri/renderD129", &info);
    tree.descriptor(201, 4, "/dev/dri/renderD128", "pos:\t0\nflags:\t0100002\n");
    tree.descriptor(201, 5, "/dev/null", &info.replace("client-id:\t9", "client-id:\t10"));
    tree
}

#[test]
fn an_amd_card_lists_each_client_once_and_primes_its_busy_figure() {
    let tree = amd_desktop(0);
    let mut sampler = ProcessSampler::open_in(tree.root(), &tree.root().join("drm"));
    sampler.resolve = entry;
    assert_eq!(sampler.cards().len(), 1);
    assert_eq!(sampler.cards()[0].pci_slot.as_deref(), Some("0000:0c:00.0"));
    assert!(sampler.graphics("RTX 9999", 10).is_none());

    let first = sampler.graphics("AMD card0", 10).unwrap();
    assert!(first.busy_priming);
    assert_eq!(first.applications.len(), 1);
    let browser = &first.applications[0];
    assert_eq!(browser.value, CardUse { busy: None, vram: Some(64 * 1024 * 1024) });
    assert_eq!(browser.members.len(), 1, "the shared client is credited once, to its opener");
    assert_eq!(browser.members[0].key.pid, 100);

    let tree = amd_desktop(50_000_000);
    let mut fresh = ProcessSampler::open_in(tree.root(), &tree.root().join("drm"));
    fresh.resolve = entry;
    fresh.graphics("AMD card0", 10);
    let baseline = fresh.graphics.get_mut(&0).unwrap();
    baseline.at = Some(Instant::now() - Duration::from_millis(100));
    for counters in baseline.engines.values_mut() {
        counters[0].1 = 0;
    }
    let second = fresh.graphics("AMD card0", 10).unwrap();
    assert!(!second.busy_priming);
    let busy = second.applications[0].value.busy.unwrap();
    assert!(busy > 0.3 && busy <= 0.5, "50 ms of work in about 100 ms, got {busy}");
}

#[test]
fn nvidia_rows_read_listed_idle_processes_as_zero_and_take_the_newest_sample() {
    let sample = |pid, timestamp, sm_util| ProcessUtilizationSample {
        pid,
        timestamp,
        sm_util,
        mem_util: 0,
        enc_util: 0,
        dec_util: 0,
    };
    let samples = [sample(7, 10, 40), sample(7, 30, 70), sample(9, 20, 15)];
    let (rows, newest) = nvidia_rows(&[(7, Some(200)), (7, Some(200)), (8, None)], Some(&samples));
    assert!(close(rows[&7].busy.unwrap(), 0.7));
    assert_eq!(rows[&7].vram, Some(200));
    assert_eq!(rows[&8], CardUse { busy: Some(0.0), vram: None });
    assert!(close(rows[&9].busy.unwrap(), 0.15));
    assert_eq!(newest, Some(30));

    let (unsupported, newest) = nvidia_rows(&[(8, Some(1))], None);
    assert_eq!(unsupported[&8], CardUse { busy: None, vram: Some(1) });
    assert_eq!(newest, None);
}

#[test]
fn nvml_and_drm_spell_one_pci_address_the_same_way_after_shortening() {
    assert_eq!(short_slot("00000000:01:00.0"), "0000:01:00.0");
    assert_eq!(short_slot("0000:0C:00.0"), "0000:0c:00.0");
}

#[test]
fn a_version_number_for_a_binary_gives_way_to_its_install_directory() {
    let tree = ProcTree::new();
    tree.process(401, "2.1.252", 1, 0, 5);
    tree.exe(401, "/home/someone/.local/share/claude/versions/2.1.252");
    tree.pages(401, 100_000, 0);
    let fold = sampler(&tree).memory(10);
    assert_eq!(fold.applications[0].name, "claude");
    assert!(looks_like_version("2.1.252"));
    assert!(!looks_like_version("2.1.252-beta"));
}
