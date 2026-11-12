//! Service tests against the in-memory driver. Nothing here touches a real GPU.

use super::mock::*;
use super::*;

struct World {
    drs: MockDrs,
    nv: NvidiaProfiles,
    _dir: tempfile::TempDir,
    _home: (std::sync::MutexGuard<'static, ()>, tempfile::TempDir),
}

fn world_with(drs: MockDrs) -> World {
    let home = crate::minecraft::test_home();
    let dir = tempfile::tempdir().unwrap();
    World { nv: NvidiaProfiles::new(Arc::new(drs.clone()), dir.path().join("backups")), drs, _dir: dir, _home: home }
}

fn world() -> World {
    world_with(MockDrs::typical())
}

const CS2: &str = "Counter-Strike 2";
const CYBER: &str = "Cyberpunk 2077";
const INDIE: &str = "My Indie Game";

#[test]
fn missing_driver_is_reported_not_fatal() {
    let w = world_with(MockDrs::without_driver("nvapi64.dll was not found. Install the NVIDIA graphics driver."));
    let st = w.nv.status();
    assert!(!st.available);
    assert!(st.reason.unwrap().contains("nvapi64.dll"));
    assert!(w.nv.list(false).unwrap_err().to_string().contains("nvapi64.dll"));
    assert!(w.nv.detail("x").is_err());
    assert!(w.nv.apply("x", &[Change::RestoreProfile], None).is_err());
    // Files can still be inspected and compared without a driver.
    let p = w.nv.import_preview(include_bytes!("fixtures/npi_current.nip")).unwrap();
    assert!(!p.driver_available && p.driver_reason.is_some());
    assert_eq!(p.profiles.len(), 2);
    assert!(p.profiles.iter().all(|x| !x.exists));
    assert!(w.nv.backups().is_empty());
}

#[test]
fn drs_session_failure_surfaces_with_the_operation_name() {
    let w = world();
    w.drs.0.lock().fail_on = Some("open".into());
    assert!(w.nv.list(true).unwrap_err().to_string().contains("NvAPI_DRS_CreateSession"));
}

#[test]
fn profiles_are_enumerated_with_state() {
    let w = world();
    let st = w.nv.status();
    assert!(st.available);
    assert_eq!(st.driver.version.as_deref(), Some("566.14"));
    let list = w.nv.list(false).unwrap();
    assert_eq!(list.len(), 4);
    assert_eq!(list[0].kind, ProfileKind::Global, "the global profile is listed first");
    let cs = list.iter().find(|p| p.name == CS2).unwrap();
    assert_eq!((cs.kind, cs.customised_count, cs.executables.clone()), (ProfileKind::Predefined, 1, vec!["cs2.exe".to_string()]));
    assert!(cs.is_customised());
    assert!(!list.iter().find(|p| p.name == CYBER).unwrap().is_customised());
    let indie = list.iter().find(|p| p.name == INDIE).unwrap();
    assert_eq!(indie.kind, ProfileKind::User);
    assert!(indie.is_customised());
    assert_eq!(w.nv.customised_names().unwrap(), vec![CS2.to_string(), INDIE.to_string()]);
}

#[test]
fn settings_are_read_with_origin_labels_and_raw_values() {
    let w = world();
    let d = w.nv.detail(CS2).unwrap();
    let vsync = d.settings.iter().find(|s| s.id == VSYNC).unwrap();
    assert_eq!(vsync.state, SettingState::User);
    assert!(vsync.modified && vsync.editable && vsync.documented && vsync.exposed);
    assert_eq!(vsync.raw, "0x08416747");
    assert_eq!(vsync.id_hex, "0x00A879CF");
    assert!(vsync.value_label.is_some(), "a friendly label is shown, next to the raw value");
    // Restoring it would bring back what the global profile says.
    assert_eq!(vsync.default, Some(Value::Dword(0x60925292)));
    let pre = d.settings.iter().find(|s| s.id == PRERENDER).unwrap();
    assert_eq!((pre.state, pre.modified), (SettingState::Nvidia, false));
    let power = d.settings.iter().find(|s| s.id == POWER_MODE).unwrap();
    assert_eq!(power.state, SettingState::Inherited);
    assert_eq!(power.value, Some(Value::Dword(5)));
    // A curated setting this driver has no value for is listed as unset, never with a made-up value.
    let unset = d.settings.iter().find(|s| s.state == SettingState::Unset).unwrap();
    assert!(unset.value.is_none() && unset.raw.is_empty());
    assert!(d.groups.len() > 2);

    // A setting nothing can name keeps its id and value and is flagged as undocumented.
    let d = w.nv.detail(INDIE).unwrap();
    let odd = d.settings.iter().find(|s| s.id == 0xDEADBEEF).unwrap();
    assert_eq!((odd.name.as_str(), odd.documented, odd.raw.as_str()), ("", false, "0x00000007"));
    assert_eq!(odd.group, meta::UNKNOWN_GROUP);
    assert!(w.nv.detail("No Such Profile").unwrap_err().to_string().contains("no profile named"));
}

#[test]
fn encoded_nvidia_values_are_read_only() {
    let w = world();
    let d = w.nv.detail(CYBER).unwrap();
    assert!(d.internal_scan);
    let enc = d.settings.iter().find(|s| s.id == 0x00F00BA5).unwrap();
    assert!(enc.internal && !enc.editable);
    assert_eq!(enc.raw, "0x0000CAFE", "shown exactly as the driver returned it");
    assert!(w.nv.plan(CYBER, &[Change::Set { id: 0x00F00BA5, value: Value::Dword(1) }]).unwrap_err().to_string().contains("encoded"));
}

#[test]
fn edit_is_snapshotted_saved_verified_and_restorable() {
    let w = world();
    let changes = vec![Change::Set { id: PRERENDER, value: Value::Dword(3) }, Change::Set { id: TEXT_SETTING, value: Value::Wstring("hello".into()) }];
    // The preview shows the intended differences and writes nothing.
    let plan = w.nv.plan(CS2, &changes).unwrap();
    assert_eq!(plan.len(), 2);
    assert!(plan[0].before.contains("0x00000001") && plan[0].after.contains("0x00000003"));
    assert_eq!(w.drs.0.lock().saves, 0);
    assert!(w.nv.backups().is_empty());

    let out = w.nv.apply(CS2, &changes, None).unwrap();
    assert!(out.verified, "{:?}", out.mismatches);
    assert_eq!(w.drs.0.lock().saves, 1);
    assert_eq!(w.drs.user_value(CS2, PRERENDER), Some(Value::Dword(3)));
    assert_eq!(w.drs.user_value(CS2, TEXT_SETTING), Some(Value::Wstring("hello".into())));
    let b = &w.nv.backups()[0];
    assert_eq!((b.verified, b.reason, b.changes.len()), (Some(true), BackupReason::Edit, 2));
    assert_eq!(w.nv.list(false).unwrap().iter().find(|p| p.name == CS2).unwrap().customised_count, 3);

    // One click puts the previous state back, and that restore is itself recorded.
    let r = w.nv.restore_backup(&out.backup_id).unwrap();
    assert!(r.verified, "{:?}", r.mismatches);
    assert_eq!(w.drs.user_value(CS2, PRERENDER), None);
    assert_eq!(w.drs.user_value(CS2, TEXT_SETTING), None);
    assert_eq!(w.drs.user_value(CS2, VSYNC), Some(Value::Dword(0x08416747)), "the earlier user value survives");
    assert_eq!(w.nv.backups().len(), 2);
    assert_eq!(w.nv.backups()[0].reason, BackupReason::Restore);
}

#[test]
fn restore_single_setting_and_whole_profile() {
    let w = world();
    let out = w.nv.apply(CS2, &[Change::Restore { id: VSYNC }], None).unwrap();
    assert!(out.verified);
    assert!(out.changes[0].after.contains("global profile"));
    assert_eq!(w.drs.user_value(CS2, VSYNC), None);
    assert!(w.nv.plan(CS2, &[Change::Restore { id: VSYNC }]).unwrap_err().to_string().contains("no user value"));

    // A user-made profile keeps existing; its settings go.
    let out = w.nv.apply(INDIE, &[Change::RestoreProfile], None).unwrap();
    assert!(out.verified);
    let d = w.nv.detail(INDIE).unwrap();
    assert_eq!(d.summary.customised_count, 0);
    assert_eq!(d.apps.len(), 1);
}

#[test]
fn invalid_edits_are_refused_before_anything_is_written() {
    let w = world();
    let bad = [
        Change::Set { id: PRERENDER, value: Value::Wstring("text into a number".into()) },
        Change::Set { id: 0, value: Value::Dword(1) },
        Change::Set { id: TEXT_SETTING, value: Value::Wstring("x".repeat(5000)) },
        Change::AddApp { exe: "..\\..\\evil.exe".into(), find_file: None },
        Change::AddApp { exe: "cyberpunk2077.exe".into(), find_file: None },
        Change::RemoveApp { exe: "not-there.exe".into() },
    ];
    for c in bad {
        assert!(w.nv.apply(CS2, std::slice::from_ref(&c), None).is_err(), "{c:?} should be refused");
    }
    assert!(w.nv.apply(CS2, &[Change::Set { id: PRERENDER, value: Value::Dword(1) }, Change::Set { id: PRERENDER, value: Value::Dword(2) }], None).is_err());
    assert!(w.nv.apply(CS2, &[], None).is_err());
    assert_eq!(w.drs.0.lock().saves, 0);
    assert!(w.nv.backups().is_empty(), "refused before the snapshot step");
}

#[test]
fn a_failed_apply_changes_nothing_in_the_driver() {
    let w = world();
    w.drs.0.lock().fail_on = Some("save".into());
    let e = w.nv.apply(CS2, &[Change::Set { id: PRERENDER, value: Value::Dword(4) }], None).unwrap_err().to_string();
    assert!(e.contains("Nothing was changed"), "{e}");
    assert_eq!(w.drs.user_value(CS2, PRERENDER), None);
    // The snapshot exists and says the change was not applied.
    let b = &w.nv.backups()[0];
    assert_eq!(b.verified, None);
    assert!(b.note.as_ref().unwrap().contains("Not applied"));
}

#[test]
fn a_save_that_does_not_stick_fails_verification() {
    let w = world();
    w.drs.0.lock().lose_saves = true;
    let out = w.nv.apply(CS2, &[Change::Set { id: PRERENDER, value: Value::Dword(4) }], None).unwrap();
    assert!(!out.verified);
    assert!(out.mismatches[0].contains("reads back"), "{:?}", out.mismatches);
    assert_eq!(w.nv.backups()[0].verified, Some(false));
}

#[test]
fn executables_can_be_added_and_removed() {
    let w = world();
    let out = w.nv.apply(INDIE, &[Change::AddApp { exe: "indie-dx12.exe".into(), find_file: Some("steam_api64.dll".into()) }, Change::RemoveApp { exe: "indie.exe".into() }], None).unwrap();
    assert!(out.verified, "{:?}", out.mismatches);
    let d = w.nv.detail(INDIE).unwrap();
    assert_eq!(d.apps.iter().map(|a| (a.name.as_str(), a.file_in_folder.as_str())).collect::<Vec<_>>(), vec![("indie-dx12.exe", "steam_api64.dll")]);
    w.nv.restore_backup(&out.backup_id).unwrap();
    assert_eq!(w.nv.detail(INDIE).unwrap().apps.iter().map(|a| a.name.clone()).collect::<Vec<_>>(), vec!["indie.exe"]);
}

#[test]
fn profiles_can_be_created_and_deleted_but_not_nvidias() {
    let w = world();
    let out = w.nv.create_profile("New Game", Some("newgame.exe")).unwrap();
    assert!(out.verified, "{:?}", out.mismatches);
    assert_eq!(w.nv.detail("New Game").unwrap().summary.kind, ProfileKind::User);
    assert!(w.nv.create_profile("new game", None).unwrap_err().to_string().contains("already exists"));
    assert!(w.nv.create_profile("Other", Some("cs2.exe")).unwrap_err().to_string().contains(CS2));
    assert!(w.nv.create_profile("  ", None).is_err());
    // Restoring the "before creating" snapshot removes the profile again.
    w.nv.restore_backup(&out.backup_id).unwrap();
    assert!(w.nv.detail("New Game").is_err());

    assert!(w.nv.delete_profile(CYBER).unwrap_err().to_string().contains("NVIDIA's own"));
    assert!(w.nv.delete_profile("Base Profile").is_err());
    let out = w.nv.delete_profile(INDIE).unwrap();
    assert!(out.verified);
    assert!(w.nv.detail(INDIE).is_err());
    // …and the deleted profile comes back from its snapshot, settings and all.
    let r = w.nv.restore_backup(&out.backup_id).unwrap();
    assert!(r.verified, "{:?}", r.mismatches);
    assert_eq!(w.drs.user_value(INDIE, 0xDEADBEEF), Some(Value::Dword(7)));
}

#[test]
fn export_writes_user_changes_in_profile_inspector_format() {
    let w = world();
    let bytes = w.nv.export(&[CS2.to_string(), INDIE.to_string()]).unwrap();
    assert!(bytes.starts_with(&[0xEF, 0xBB, 0xBF]));
    let f = nip::parse(&bytes).unwrap();
    assert_eq!(f.profiles.len(), 2);
    // Only what the user changed is exported; NVIDIA's own values are not.
    assert_eq!(f.profiles[0].settings.iter().map(|s| s.id).collect::<Vec<_>>(), vec![VSYNC]);
    assert_eq!(f.profiles[0].settings[0].name_info.as_deref(), Some("Vertical Sync"));
    assert_eq!(f.profiles[0].executables, vec!["cs2.exe"]);
    // A setting Arcadia cannot name is exported like any other.
    assert!(f.profiles[1].settings.iter().any(|s| s.id == 0xDEADBEEF && s.raw_value == "7"));

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("out.nip");
    assert_eq!(w.nv.export_to(&path, &[CS2.to_string()]).unwrap(), 1);
    assert_eq!(nip::parse(&std::fs::read(&path).unwrap()).unwrap().profiles[0].name, CS2);
    assert!(w.nv.export_to(&dir.path().join("out.txt"), &[CS2.to_string()]).is_err());
    assert!(w.nv.export_to(std::path::Path::new("relative.nip"), &[CS2.to_string()]).is_err());
    assert!(w.nv.export(&["Nope".to_string()]).is_err());
    assert!(w.nv.export(&[]).is_err());
}

#[test]
fn import_preview_compares_the_file_with_the_live_driver() {
    let w = world();
    let mut file = NipFile::new(vec![NipProfile {
        name: CS2.into(),
        executables: vec!["cs2.exe".into(), "cyberpunk2077.exe".into()],
        settings: vec![
            NipSetting::new(VSYNC, "Vertical Sync", &Value::Dword(0x08416747)),
            NipSetting::new(PRERENDER, "", &Value::Dword(4)),
            NipSetting { id: 555, name_info: Some("Future".into()), value_type: "Float".into(), raw_value: "1.5".into(), type_absent: false },
            NipSetting::new(TEXT_SETTING, "", &Value::Dword(1)),
        ],
        ..Default::default()
    }]);
    file.profiles.push(NipProfile { name: "Brand New".into(), ..Default::default() });
    let p = w.nv.import_preview(&nip::write(&file)).unwrap();
    assert!(p.driver_available);
    let cs = &p.profiles[0];
    assert!(cs.exists);
    assert_eq!(cs.kind, Some(ProfileKind::Predefined));
    assert_eq!(cs.conflicts, vec![("cyberpunk2077.exe".to_string(), CYBER.to_string())]);
    assert!(cs.settings[0].same_as_live);
    assert!(!cs.settings[1].same_as_live && cs.settings[1].live.as_ref().unwrap().contains("0x00000001"));
    assert!(cs.settings[2].problem.as_ref().unwrap().contains("not known"));
    assert!(cs.settings[3].problem.is_none(), "type conflicts with settings the profile does not store are caught on apply");
    assert!(cs.live_only.is_empty());
    assert!(!p.profiles[1].exists);
    assert!(p.warnings.iter().any(|x| x.contains("cannot be applied")));
    assert_eq!(w.drs.0.lock().saves, 0);
}

#[test]
fn import_replace_and_merge_differ_only_in_what_they_remove() {
    let file = NipFile::new(vec![NipProfile { name: CS2.into(), executables: vec!["cs2.exe".into(), "cs2-vulkan.exe".into()], settings: vec![NipSetting::new(PRERENDER, "", &Value::Dword(2))], ..Default::default() }]);

    let w = world();
    let out = w.nv.import(&file, &[], ImportMode::Merge, "Merge test").unwrap();
    assert!(out.verified, "{:?}", out.mismatches);
    assert_eq!(w.drs.user_value(CS2, PRERENDER), Some(Value::Dword(2)));
    assert_eq!(w.drs.user_value(CS2, VSYNC), Some(Value::Dword(0x08416747)), "merge keeps existing user values");

    drop(w);
    let w = world();
    let out = w.nv.import(&file, &[], ImportMode::Replace, "Replace test").unwrap();
    assert!(out.verified, "{:?}", out.mismatches);
    assert_eq!((out.profiles[0].ok, out.profiles[0].applied, out.profiles[0].created), (true, 1, false));
    assert_eq!(w.drs.user_value(CS2, PRERENDER), Some(Value::Dword(2)));
    assert_eq!(w.drs.user_value(CS2, VSYNC), None, "replace removes user values that are not in the file");
    let apps: Vec<String> = w.nv.detail(CS2).unwrap().apps.into_iter().map(|a| a.name).collect();
    assert_eq!(apps, vec!["cs2.exe", "cs2-vulkan.exe"]);
    let b = &w.nv.backups()[0];
    assert_eq!((b.reason, b.verified), (BackupReason::Import, Some(true)));
    assert!(b.changes.iter().any(|c| c.after.contains("not in the file")));

    // Restoring the pre-import snapshot brings the old state back exactly.
    w.nv.restore_backup(&out.backup_id).unwrap();
    assert_eq!(w.drs.user_value(CS2, VSYNC), Some(Value::Dword(0x08416747)));
    assert_eq!(w.drs.user_value(CS2, PRERENDER), None);
    assert_eq!(w.nv.detail(CS2).unwrap().apps.len(), 1);
}

#[test]
fn import_creates_profiles_and_skips_what_the_driver_refuses() {
    let w = world();
    w.drs.0.lock().unknown_to_driver.insert(0x0BADF00D);
    let file = NipFile::new(vec![NipProfile {
        name: "Imported Game".into(),
        executables: vec!["imported.exe".into()],
        find_files: vec![nip::FindFile { executable: Some("imported.exe".into()), find_file: Some("marker.dll".into()) }],
        settings: vec![
            NipSetting::new(VSYNC, "", &Value::Dword(0x47814940)),
            NipSetting::new(0x0BADF00D, "Gone in this driver", &Value::Dword(1)),
            NipSetting::new(PRERENDER, "", &Value::Wstring("wrong type".into())),
            NipSetting { id: 9, name_info: None, value_type: "Dword".into(), raw_value: "99999999999".into(), type_absent: false },
            NipSetting::new(BINARY_SETTING, "", &Value::Binary(vec![1, 2, 3])),
        ],
        ..Default::default()
    }]);
    let out = w.nv.import(&file, &[], ImportMode::Replace, "Create test").unwrap();
    let r = &out.profiles[0];
    assert!(r.ok && r.created);
    assert_eq!(r.applied, 2);
    assert_eq!(r.skipped.len(), 3);
    assert!(r.skipped.iter().any(|s| s.id == 0x0BADF00D && s.reason.contains("does not know")));
    assert!(out.verified, "{:?}", out.mismatches);
    let d = w.nv.detail("Imported Game").unwrap();
    assert_eq!(d.apps[0].file_in_folder, "marker.dll");
    assert_eq!(w.drs.user_value("Imported Game", BINARY_SETTING), Some(Value::Binary(vec![1, 2, 3])));
    // Undoing the import removes the profile it created.
    w.nv.restore_backup(&out.backup_id).unwrap();
    assert!(w.nv.detail("Imported Game").is_err());
}

#[test]
fn a_profile_that_cannot_be_imported_is_left_untouched_and_others_still_land() {
    let w = world();
    let file = NipFile::new(vec![
        NipProfile { name: INDIE.into(), executables: vec!["cs2.exe".into()], settings: vec![NipSetting::new(VSYNC, "", &Value::Dword(1))], ..Default::default() },
        NipProfile { name: CYBER.into(), settings: vec![NipSetting::new(VSYNC, "", &Value::Dword(0x47814940))], ..Default::default() },
    ]);
    let out = w.nv.import(&file, &[], ImportMode::Replace, "Conflict test").unwrap();
    assert!(!out.verified);
    assert!(!out.profiles[0].ok);
    assert!(out.profiles[0].error.as_ref().unwrap().contains(CS2));
    // The failed profile's session was discarded: its old settings are still there.
    assert_eq!(w.drs.user_value(INDIE, POWER_MODE), Some(Value::Dword(1)));
    assert_eq!(w.drs.user_value(INDIE, VSYNC), None);
    assert!(out.profiles[1].ok);
    assert_eq!(w.drs.user_value(CYBER, VSYNC), Some(Value::Dword(0x47814940)));
    // `only` limits an import to chosen profiles.
    drop(w);
    let w = world();
    let out = w.nv.import(&file, &[CYBER.to_string()], ImportMode::Merge, "Subset").unwrap();
    assert_eq!(out.profiles.len(), 1);
    assert!(w.nv.import(&file, &["Not In File".to_string()], ImportMode::Merge, "x").is_err());
}

#[test]
fn an_edited_file_is_validated_again_before_it_is_applied() {
    let w = world();
    let mut file = NipFile::new(vec![NipProfile { name: CS2.into(), executables: vec!["..\\..\\evil.exe".into()], ..Default::default() }]);
    assert!(w.nv.import(&file, &[], ImportMode::Merge, "x").unwrap_err().to_string().contains(".."));
    file.profiles[0].executables.clear();
    file.profiles[0].name = String::new();
    assert!(w.nv.import(&file, &[], ImportMode::Merge, "x").is_err());
    assert_eq!(w.drs.0.lock().saves, 0);
}

#[test]
fn real_profile_inspector_fixture_imports_and_exports_back() {
    let w = world();
    let preview = w.nv.import_preview(include_bytes!("fixtures/npi_current.nip")).unwrap();
    let out = w.nv.import(&preview.file, &[], ImportMode::Replace, "Fixture").unwrap();
    assert!(out.profiles.iter().all(|p| p.ok), "{:?}", out.profiles);
    assert!(out.verified, "{:?}", out.mismatches);
    // What was imported comes back out unchanged (ids, types and values).
    let exported = nip::parse(&w.nv.export(&["Test Game & Co".to_string()]).unwrap()).unwrap();
    let diff = nip::compare(&preview.file.profiles[1..], &exported.profiles);
    assert!(diff.identical, "{diff:?}");
    assert_eq!(exported.profiles[0].find_file_for("testgame.exe"), Some("steam_api64.dll"));
}

#[test]
fn manual_backups_and_file_comparison() {
    let w = world();
    let e = w.nv.backup_now(&[CS2.to_string(), INDIE.to_string()], "  Before the tournament ").unwrap();
    assert_eq!((e.label.as_str(), e.reason), ("Before the tournament", BackupReason::Manual));
    assert!(w.nv.backup_path(&e.id).unwrap().is_file());
    let contents = w.nv.backup_contents(&e.id).unwrap();
    assert_eq!(contents.profiles.len(), 2);
    assert!(contents.profiles[0].settings[0].same_as_live);
    assert!(w.nv.backup_now(&[], "x").is_err());

    w.nv.apply(CS2, &[Change::Set { id: VSYNC, value: Value::Dword(0x47814940) }], None).unwrap();
    let before = std::fs::read(w.nv.backup_path(&e.id).unwrap()).unwrap();
    let after = w.nv.export(&[CS2.to_string(), INDIE.to_string()]).unwrap();
    let diff = w.nv.compare_files(&before, &after).unwrap();
    assert!(!diff.identical);
    let cs = diff.profiles.iter().find(|p| p.name == CS2).unwrap();
    assert_eq!(cs.settings.len(), 1);
    assert_eq!(cs.settings[0].name, "Vertical Sync");
    assert_eq!(diff.profiles.iter().find(|p| p.name == INDIE).unwrap().status, "same");
    assert!(w.nv.compare_files(b"junk", &after).unwrap_err().to_string().starts_with("First file"));

    w.nv.delete_backup(&e.id).unwrap();
    assert!(w.nv.restore_backup(&e.id).is_err());
}

#[test]
fn internal_flag_scan_reads_text_dumps_only() {
    let text = "Profile \"Some Game\"\n    Setting ID_0x00f00ba5 = 0x1 InternalSettingFlag=V0\n    Setting ID_0x00000001 = 0x2\nEndProfile\n";
    let set = parse_internal_flags(text.as_bytes()).unwrap();
    assert!(set.contains(&("some game".to_string(), 0x00F00BA5)) && set.len() == 1);
    let utf16: Vec<u8> = [0xFF, 0xFE].into_iter().chain(text.encode_utf16().flat_map(|u| u.to_le_bytes())).collect();
    assert_eq!(parse_internal_flags(&utf16).unwrap().len(), 1);
    assert!(parse_internal_flags(&[0u8, 159, 146, 150, 1, 2, 3]).is_none(), "a binary dump means 'cannot tell'");
}
