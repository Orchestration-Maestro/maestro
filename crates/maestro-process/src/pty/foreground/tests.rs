//! Process discovery, parsing and bounded child-group selection.
use super::*;
use std::{
    io::{Read, Write},
    os::unix::ffi::OsStringExt,
    process::{Command, Stdio},
};

#[test]
fn explicit_detection_modes_keep_the_native_default() {
    for input in [None, Some(""), Some("native")] {
        assert_eq!(
            parse_process_detection_mode(input),
            Ok(ProcessDetectionMode::Native)
        );
    }
    assert_eq!(
        parse_process_detection_mode(Some("child-groups")),
        Ok(ProcessDetectionMode::ChildGroups)
    );
    assert_eq!(parse_process_detection_mode(Some("gvisor")), Err("gvisor"));
}

#[test]
fn foreground_members_include_nested_and_thread_children_only() {
    use std::collections::HashMap;
    let children = HashMap::from([
        ((100, 100), vec![300, 221, 201, 200]),
        ((100, 101), vec![210]),
        ((200, 200), vec![220]),
        ((220, 220), vec![221]),
    ]);
    let processes = HashMap::from([
        (100, (100, "shell")),
        (200, (200, "leader")),
        (201, (200, "pipeline")),
        (210, (200, "thread-child")),
        (220, (220, "intermediate")),
        (221, (200, "nested")),
        (300, (300, "background")),
        (9999, (200, "unrelated")),
    ]);
    let members = foreground_process_group_members_with(
        100,
        200,
        |pid| {
            if pid == 100 {
                vec![100, 101]
            } else {
                vec![pid]
            }
        },
        |pid, tid| children.get(&(pid, tid)).cloned().unwrap_or_default(),
        |group, pid| {
            let (pgrp, comm) = processes.get(&pid)?;
            (*pgrp == group).then(|| ProcGroupMember {
                pid,
                comm: (*comm).into(),
            })
        },
    )
    .unwrap();
    assert_eq!(
        members
            .into_iter()
            .map(|member| (member.pid, member.comm))
            .collect::<Vec<_>>(),
        [
            (200, "leader"),
            (201, "pipeline"),
            (210, "thread-child"),
            (221, "nested")
        ]
        .map(|(pid, name)| (pid, name.to_owned()))
    );
}

#[test]
fn child_group_selection_uses_the_largest_live_group() {
    let group = child_groups_foreground_process_group_with(
        100,
        90,
        |_| vec![100],
        |_, _| vec![300, 200, 150, 0],
        |pid| match pid {
            300 => Some(300),
            200 => Some(200),
            150 => Some(90),
            _ => None,
        },
    );
    assert_eq!(group, Some(300));
}

/// Owns a separate procfs fixture tree for one test transaction.
struct ProcTree(maestro_test_directory::Scratch);
impl ProcTree {
    fn new() -> Self {
        Self(maestro_test_directory::Scratch::new().unwrap())
    }
    fn path(&self) -> &std::path::Path {
        self.0.path()
    }
    fn process(&self, pid: u32, (group, session, foreground): (i32, i32, i32), name: &str) {
        let dir = self.path().join(pid.to_string());
        std::fs::create_dir_all(dir.join(format!("task/{pid}"))).unwrap();
        std::fs::write(
            dir.join("stat"),
            format!("{pid} ({name}) S 1 {group} {session} 0 {foreground}"),
        )
        .unwrap();
        std::fs::write(dir.join("cmdline"), b"runner\0a b\0\xff\0").unwrap();
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(dir.join(format!("task/{pid}/children")))
        {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => panic!("{error}"),
        }
    }
    fn children(&self, pid: u32, text: &str) {
        std::fs::write(self.path().join(format!("{pid}/task/{pid}/children")), text).unwrap();
    }
}
#[test]
fn procfs_operations_select_live_foreground_and_session_results() {
    use super::linux::Procfs;
    let tree = ProcTree::new();
    tree.process(100, (100, 100, 200), "sh");
    tree.process(200, (200, 100, 200), "job ) name");
    tree.process(300, (300, 300, 300), "foreign");
    tree.children(100, "200 9999");
    let procfs = Procfs(tree.path());
    for mode in [
        ProcessDetectionMode::Native,
        ProcessDetectionMode::ChildGroups,
    ] {
        let job = procfs.foreground_job(100, mode).unwrap();
        assert_eq!(job.process_group_id, 200);
        assert_eq!(
            job.processes,
            vec![ForegroundProcess {
                pid: 200,
                name: "job ) name".into(),
                argv0: None,
                argv: Some(vec!["runner".into(), "a b".into(), "�".into()]),
                cmdline: Some("runner a b �".into())
            }]
        );
    }
    assert_eq!(procfs.leader(200).unwrap().processes[0].name, "job ) name");
    assert!(procfs.leader(9999).is_none());
    tree.process(200, (201, 100, 200), "wrong-group");
    assert!(procfs.leader(200).is_none());
    tree.process(100, (100, 100, -1), "sh");
    tree.process(200, (200, 100, 200), "job");
    assert!(
        procfs
            .foreground_job(100, ProcessDetectionMode::Native)
            .is_none()
    );
    assert_eq!(
        procfs
            .foreground_job(100, ProcessDetectionMode::ChildGroups)
            .unwrap()
            .process_group_id,
        200
    );
    tree.process(100, (100, 100, 500), "sh");
    assert!(
        procfs
            .foreground_job(100, ProcessDetectionMode::ChildGroups)
            .is_none()
    );
    let mut session = procfs.session_processes(100);
    session.sort_unstable();
    assert_eq!(session, [100, 200]);
    std::fs::remove_file(tree.path().join("200/stat")).unwrap();
    assert_eq!(procfs.session_processes(100), [100]);
    assert!(procfs.session_processes(9999).is_empty());
}

#[test]
fn session_members_survive_the_reaped_session_leader() {
    let tree = ProcTree::new();
    tree.process(100, (100, 100, 100), "shell");
    tree.process(200, (200, 100, 200), "survivor");
    tree.process(300, (300, 300, 300), "foreign");
    let procfs = super::linux::Procfs(tree.path());
    let mut members = procfs.session_processes_by_id(100);
    members.sort_unstable();
    assert_eq!(members, [100, 200]);
    std::fs::remove_file(tree.path().join("100/stat")).unwrap();
    assert_eq!(procfs.session_processes_by_id(100), [200]);
    assert_eq!(procfs.session_processes(200), [200]);
    assert!(procfs.session_processes_by_id(0).is_empty());
    assert!(procfs.session_processes_by_id(u32::MAX).is_empty());
}

use super::linux::{environment_value, process_argv_from_bytes, process_pgrp_and_comm_from_stat};
use std::{cell::RefCell, collections::HashMap};

#[test]
fn child_group_selection_returns_the_shell_when_no_job_survives() {
    let tasks = HashMap::from([(100, vec![100])]);
    let children = HashMap::from([((100, 100), vec![150, 160])]);
    let groups = HashMap::from([(150, 90), (160, 90)]);

    let group = child_groups_foreground_process_group_with(
        100,
        90,
        |pid| tasks.get(&pid).cloned().unwrap_or_default(),
        |pid, tid| children.get(&(pid, tid)).cloned().unwrap_or_default(),
        |pid| groups.get(&pid).copied(),
    );

    assert_eq!(group, Some(90));
}

#[test]
fn child_group_selection_filters_the_shell_group() {
    let tasks = HashMap::from([(100, vec![100])]);
    let children = HashMap::from([((100, 100), vec![150, 160, 300])]);
    let groups = HashMap::from([(150, 90), (160, 90), (300, 300)]);

    let group = child_groups_foreground_process_group_with(
        100,
        90,
        |pid| tasks.get(&pid).cloned().unwrap_or_default(),
        |pid, tid| children.get(&(pid, tid)).cloned().unwrap_or_default(),
        |pid| groups.get(&pid).copied(),
    );

    assert_eq!(group, Some(300));
}

#[test]
fn child_group_scan_fails_closed_after_sixty_four_children() {
    let children: Vec<u32> = (1..=(u32::try_from(CHILD_GROUPS_SCAN_LIMIT).unwrap() + 10)).collect();
    let mut inspected = 0usize;
    let group = child_groups_foreground_process_group_with(
        100,
        100,
        |_| vec![100],
        |_, _| children.clone(),
        |pid| {
            inspected += 1;
            Some(i32::try_from(pid).unwrap())
        },
    );
    assert_eq!(inspected, CHILD_GROUPS_SCAN_LIMIT);
    assert_eq!(group, None);
    for record in [200, 9999, 100] {
        let select = |count| {
            let mut inspected = 0;
            let group = child_groups_foreground_process_group_with(
                100,
                100,
                |_| vec![100],
                |_, _| vec![record; count],
                |pid| {
                    inspected += 1;
                    match pid {
                        200 => Some(200),
                        100 => Some(100),
                        _ => None,
                    }
                },
            );
            (group, inspected)
        };
        assert_eq!(
            select(64),
            (Some(if record == 200 { 200 } else { 100 }), 64)
        );
        assert_eq!(select(65), (None, CHILD_GROUPS_SCAN_LIMIT));
    }
}

#[test]
fn foreground_members_keep_a_direct_leader_without_tree_data() {
    let members = foreground_process_group_members_with(
        100,
        200,
        |_| Vec::new(),
        |_, _| Vec::new(),
        |process_group_id, pid| {
            (pid == process_group_id).then(|| ProcGroupMember {
                pid,
                comm: "leader".to_string(),
            })
        },
    )
    .unwrap();

    assert_eq!(
        members,
        vec![ProcGroupMember {
            pid: 200,
            comm: "leader".to_string()
        }]
    );
}

#[test]
fn foreground_members_read_fresh_children_each_time() {
    let children = RefCell::new(HashMap::from([((100, 100), vec![200])]));
    let discover = || {
        foreground_process_group_members_with(
            100,
            200,
            |pid| vec![pid],
            |pid, tid| {
                children
                    .borrow()
                    .get(&(pid, tid))
                    .cloned()
                    .unwrap_or_default()
            },
            |process_group_id, pid| {
                [200, 201]
                    .contains(&pid)
                    .then(|| ProcGroupMember {
                        pid,
                        comm: format!("member-{pid}"),
                    })
                    .filter(|_| process_group_id == 200)
            },
        )
        .unwrap()
        .into_iter()
        .map(|member| member.pid)
        .collect::<Vec<_>>()
    };

    assert_eq!(discover(), vec![200]);
    children.borrow_mut().insert((100, 100), vec![200, 201]);
    assert_eq!(discover(), vec![200, 201]);
}

#[test]
fn proc_stat_names_accept_spaces_and_closing_parentheses() {
    assert_eq!(
        process_pgrp_and_comm_from_stat("123 (name with ) paren) S 1 456 789 0 456"),
        Some((456, "name with ) paren".to_string()))
    );
}

#[test]
fn raw_process_values_keep_first_environment_record_and_native_paths() {
    if maestro_test_process::run_alone(
        "raw_process_values_keep_first_environment_record_and_native_paths",
    )
    .unwrap()
    {
        return;
    }
    for (bytes, expected) in [
        (&b"OTHER=x\0"[..], None),
        (&b"MODE=\0MODE=valid\0"[..], Some("")),
        (&b"MODE=unknown\0MODE=valid\0"[..], Some("unknown")),
        (&b"MODE=\xff\0MODE=valid\0"[..], None),
    ] {
        assert_eq!(environment_value(bytes, "MODE").as_deref(), expected);
    }
    assert_eq!(
        process_argv_from_bytes(b"\0a\0\0\xff\0"),
        Some(vec!["a".into(), "�".into()])
    );
    assert_eq!(process_argv_from_bytes(b"\0\0"), None);
    assert_eq!(process_argv_from_bytes(b""), None);
    assert_eq!(process_cwd(0), None);
    assert_eq!(process_cwd(u32::MAX), None);
    assert_eq!(process_environment_value(0, "MODE"), None);
    assert_eq!(process_environment_value(u32::MAX, "MODE"), None);
    assert_eq!(
        process_cwd(std::process::id()),
        Some(std::env::current_dir().unwrap())
    );
    assert_native_process_values();
}

#[test]
fn process_tree_discovery_scales_with_unique_reachable_pids() {
    use std::cell::Cell;
    for n in [32, 64] {
        let tasks = Cell::new(0);
        let children = Cell::new(0);
        let pids = process_tree_pids(
            [1, 1, 0],
            |pid| {
                tasks.set(tasks.get() + 1);
                vec![pid]
            },
            |pid, _| {
                children.set(children.get() + 1);
                chain_children(pid, n)
            },
        );
        assert_eq!(pids, (1..=n).collect::<Vec<_>>());
        assert_eq!((tasks.get(), children.get()), (n, n));
        let repeated_reads = Cell::new(0);
        for root in &pids {
            process_tree_pids(
                [*root, 1],
                |pid| {
                    repeated_reads.set(repeated_reads.get() + 1);
                    vec![pid]
                },
                |pid, _| chain_children(pid, n),
            );
        }
        assert_eq!(repeated_reads.get(), if n == 32 { 1024 } else { 4096 });
    }
    assert_eq!(
        process_tree_pids(
            [10, 20],
            |pid| vec![pid, pid + 1],
            |pid, tid| match (pid, tid) {
                (10, 10 | 11) => vec![20],
                (20, 20) => vec![10, 30],
                _ => vec![],
            }
        ),
        [10, 20, 30]
    );
}

/// Observes native path identity and explicit environment values after a child starts.
fn assert_native_process_values() {
    let tree = ProcTree::new();
    let target = tree
        .path()
        .join(std::ffi::OsString::from_vec(b"cwd-\xff".to_vec()));
    std::fs::create_dir(&target).unwrap();
    let authored = tree.path().join("alias");
    std::os::unix::fs::symlink(&target, &authored).unwrap();
    let mut child = Command::new("/bin/sh")
        .args(["-c", "printf READY; read -r ignored"])
        .env("MAESTRO_PROCESS_VALUE", "")
        .current_dir(authored)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut ready = [0; 5];
    child.stdout.take().unwrap().read_exact(&mut ready).unwrap();
    assert_eq!(&ready, b"READY");
    assert_eq!(process_cwd(child.id()), Some(target));
    assert_eq!(
        process_environment_value(child.id(), "MAESTRO_PROCESS_VALUE").as_deref(),
        Some("")
    );
    child.stdin.take().unwrap().write_all(b"go\n").unwrap();
    assert!(child.wait().unwrap().success());
    assert_eq!(process_cwd(child.id()), None);
}

/// Supplies one cyclic graph to the unique-walk and repeated-scope controls.
fn chain_children(pid: u32, limit: u32) -> Vec<u32> {
    if pid < limit {
        vec![pid + 1, 1, 0]
    } else {
        vec![1]
    }
}
