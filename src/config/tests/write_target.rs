//! One save, one file — even when the path stops naming it.
//!
//! A config path may be a symlink, and so may any directory above it; the loader
//! follows the chain, resolves the directories the chain ends in, reads the bytes
//! at that file, and records what it walked. Everything the save does afterwards
//! is about *that* file: the advisory lock is taken for it, the two byte
//! comparisons are made against it, and the rename lands on it. Resolving the
//! path a second time at any of those steps would undo the agreement — a link
//! retargeted while the window is open would move the write to a file no lock
//! covers and no comparison ever read, and the bytes it already held would be
//! replaced by a merge of somebody else's file.
//!
//! Two halves. A retarget the save can still see is a stale source, reported so
//! the editors reload and reapply onto whatever the path names now. A retarget
//! that arrives after the last comparison cannot be seen at all, and the pinned
//! destination is what makes it harmless: the write lands where the window was,
//! and the file the link now points at is left exactly as it was.
//!
//! The mirror image is a name that never moves while the *file* under it does.
//! Only the writers that take the advisory lock are held off, and an editor
//! outside this application takes nothing: it can rename the checked
//! `config.toml` away and leave a file of its own at that name, inside the same
//! last stretch. Pinning the destination does not help there — the pinned path
//! is exactly where the replacement is — so the identity of the file that was
//! checked is carried down to the rename, which refuses to land on any other.
//! Both are stale sources, and both are recovered the same way: reload, reapply.

use super::super::io::{is_stale_source_error, persist_preset_slot_at};
use super::super::{ColorSpec, ConfigDocument, ToolPresetConfig};
use crate::draw::Color;
use std::fs::{self, File, OpenOptions};
use std::os::unix::fs::{MetadataExt, symlink};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

/// Long enough that the waiting editor is provably still inside its wait, and
/// far inside the save's own five-second lock deadline.
const HOLD: Duration = Duration::from_millis(150);

/// Hand-authored text no writer may lose.
const ORIGINAL: &str = "\
# Wayscriber configuration. Neither file may lose this.
[ui]
setting_from_a_later_release = 7
";

fn preset(name: &str) -> ToolPresetConfig {
    ToolPresetConfig {
        name: Some(name.to_string()),
        tool: crate::input::Tool::Pen,
        color: ColorSpec::from(Color {
            r: 0.13,
            g: 0.27,
            b: 0.41,
            a: 1.0,
        }),
        size: 7.0,
        tool_settings: None,
        eraser_kind: None,
        eraser_mode: None,
        marker_opacity: None,
        fill_enabled: None,
        font_size: None,
        text_background_enabled: None,
        arrow_length: None,
        arrow_angle: None,
        arrow_head_at_end: None,
        polygon_sides: None,
        show_status_bar: None,
        drag_tools: None,
    }
}

/// The shape every test here starts from: a config path that is a link to one
/// file, with a second file standing by to be retargeted onto.
struct Linked {
    link: PathBuf,
    original: PathBuf,
    replacement: PathBuf,
}

fn linked(directory: &Path, replacement_text: &str) -> Linked {
    fs::create_dir_all(directory).expect("the fixture directory this test named");
    let original = directory.join("original.toml");
    let replacement = directory.join("replacement.toml");
    let link = directory.join("config.toml");
    fs::write(&original, ORIGINAL).expect("the fixture this test named a directory for");
    fs::write(&replacement, replacement_text).expect("the second fixture");
    symlink(&original, &link).expect("a link this test's own directory can hold");
    Linked {
        link,
        original,
        replacement,
    }
}

/// Which file a name holds, as the save's own comparison sees it.
fn inode_of(path: &Path) -> u64 {
    fs::metadata(path)
        .expect("a fixture this test created")
        .ino()
}

fn retarget(linked: &Linked) {
    fs::remove_file(&linked.link).expect("the link this test created");
    symlink(&linked.replacement, &linked.link).expect("the retarget this test stages");
}

/// The lock a save takes, named here rather than reached for through the
/// writer's own module: it is the rendezvous two *processes* have to agree on,
/// and the agreement is about the resolved file, not the path that named it.
///
/// Resolved the whole way, directories included, because that is the file the
/// save addresses — two editors reaching one config through different links, or
/// through a directory link one of them has already resolved, still have to
/// contend for the same lock.
fn hold_config_write_lock(destination: &Path) -> File {
    let destination = &fs::canonicalize(destination).expect("a fixture this test created");
    let name = destination
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .expect("the fixture path this test built has a file name");
    let path = destination.with_file_name(format!("{name}.lock"));
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .expect("the lock file beside a fixture this test created");
    crate::session::try_lock_exclusive(&file)
        .expect("nothing else can hold the lock on a directory this test just made");
    file
}

/// A config path the working directory does not have yet, named relatively.
///
/// It cannot live under a temp directory, because a relative path that reaches
/// one runs through ancestors that do exist and the ancestor walk stops at the
/// first of them — the case here is the one where nothing on the path can be
/// resolved at all, which only a relative path whose very first component is
/// missing reaches. So the tree is made where the process already stands, under
/// a name carrying this suite's pid so a run beside another does not meet it,
/// and the guard takes the whole tree away again — the config, the `.bak`, the
/// lock file, and the directories themselves — whether the test passes or
/// panics.
struct RelativeRoot {
    relative: PathBuf,
    absolute: PathBuf,
}

impl RelativeRoot {
    fn new(label: &str) -> Self {
        let name = format!("wayscriber-test-{label}-{}", std::process::id());
        let absolute = fs::canonicalize(".")
            .expect("the working directory this suite was started in")
            .join(&name);
        // A leftover from a run that died before its guard would hide the very
        // case this is for, by handing the walk an ancestor to stop at.
        let _ = fs::remove_dir_all(&absolute);
        assert!(
            !absolute.exists(),
            "nothing may stand at {} before this test makes it",
            absolute.display()
        );
        Self {
            relative: PathBuf::from(name),
            absolute,
        }
    }
}

impl Drop for RelativeRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.absolute);
    }
}

/// The window's last stretch: a retarget after the final comparison, which no
/// check can catch, and which the pinned destination makes harmless.
///
/// The write must land on the file the document read, the file the lock was
/// taken for, and the file the comparisons were about. Resolving the config path
/// again at the rename sends it to the replacement instead: bytes nobody
/// compared, under a lock nobody holds, replaced by a merge of a different
/// file's contents.
#[test]
fn a_retarget_after_the_last_check_still_writes_the_file_the_window_was_about() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    const BYSTANDER: &str = "# A file this save has nothing to do with.\n";
    let fixture = linked(temp.path(), BYSTANDER);

    let document = ConfigDocument::load_from_path(&fixture.link).expect("load through the link");
    let mut edited = document.config().clone();
    edited.presets.set_slot(1, Some(preset("Pinned")));

    let mut retargeted = false;
    document
        .save_with_backup_racing_the_write(edited, &mut || {
            retarget(&fixture);
            retargeted = true;
        })
        .expect("a save whose own file never moved must succeed");

    assert!(retargeted, "the window must have been raced at all");
    assert_eq!(
        fs::read_link(&fixture.link).expect("the link this test retargeted"),
        fixture.replacement,
        "the retarget must really have happened before the write"
    );

    let written = fs::read_to_string(&fixture.original).expect("readable");
    assert!(
        written.contains("[presets.slot_1]"),
        "the edit must land on the file the window was about: {written}"
    );
    assert!(
        written.contains("setting_from_a_later_release = 7"),
        "and must not lose what that file already said: {written}"
    );
    assert_eq!(
        fs::read_to_string(&fixture.replacement).expect("readable"),
        BYSTANDER,
        "the file the link now points at was never part of this save"
    );
}

/// The same stretch, with the name standing still and the file moving.
///
/// The advisory lock binds the writers that take it — every one of this
/// application's — and an editor outside it takes nothing. So it can rename the
/// checked `config.toml` away and leave a file of its own under that name after
/// the save's last comparison, and nothing the save has looked at has changed:
/// the path resolves where it did, the directories are the same, and the bytes
/// it compared were read before any of this. Pinning the destination is no help
/// here, because the destination is precisely where the replacement now sits.
///
/// A rename that asked only about the name would put a merge of the *old*
/// file's text over the new one, destroying an edit nobody here ever read, and
/// report a clean save. The identity of the file that was checked is what the
/// rename is made conditional on, so the save is refused instead — and refused
/// in the wording that sends the editors round again, because reloading and
/// reapplying onto the file that is there now is exactly the right recovery.
#[test]
fn a_file_swapped_in_under_the_checked_name_is_refused_rather_than_overwritten() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    const REPLACEMENT: &str = "# Another editor's file, left at the same name.\n";
    let directory = temp.path().join("swapped");
    fs::create_dir_all(&directory).expect("the fixture directory this test named");
    let path = directory.join("config.toml");
    let moved = directory.join("moved-away.toml");
    fs::write(&path, ORIGINAL).expect("the fixture this test named a directory for");

    let document = ConfigDocument::load_from_path(&path).expect("load");
    let checked = inode_of(&path);
    let mut edited = document.config().clone();
    edited.presets.set_slot(1, Some(preset("Never written")));

    let mut swapped = false;
    let error = document
        .save_with_backup_racing_the_write(edited, &mut || {
            fs::rename(&path, &moved).expect("the file this save checked");
            fs::write(&path, REPLACEMENT).expect("the replacement this test stages");
            swapped = true;
        })
        .expect_err("the file this save checked is no longer the one at this name");

    assert!(swapped, "the window must have been raced at all");
    assert_ne!(
        inode_of(&path),
        checked,
        "the replacement must really be a different file, or this test is about nothing"
    );
    assert!(
        is_stale_source_error(&error),
        "the editors' retry gate must recognise this: {error:#}"
    );
    assert_eq!(
        fs::read_to_string(&path).expect("readable"),
        REPLACEMENT,
        "the file another editor left here keeps every byte"
    );
    assert_eq!(
        fs::read_to_string(&moved).expect("readable"),
        ORIGINAL,
        "and the file this save checked is not edited wherever it ended up"
    );

    // The recovery, through the editors' own path: a fresh load reads the file
    // that is there now, and the edit lands on top of what it says.
    persist_preset_slot_at(&path, 1, Some(&preset("Reapplied")))
        .expect("a replaced file is a reload, not a failure");

    let written = fs::read_to_string(&path).expect("readable");
    assert!(
        written.contains("[presets.slot_1]"),
        "the edit must be reapplied onto the file the name holds now: {written}"
    );
    assert!(
        written.contains("Another editor's file"),
        "onto what that file already held, not over it: {written}"
    );
    assert_eq!(
        fs::read_to_string(&moved).expect("readable"),
        ORIGINAL,
        "and the file the save was originally about keeps every byte"
    );
}

/// The same file can change without its identity changing.
///
/// Some editors truncate and rewrite in place rather than replacing the file.
/// Device and inode still agree with the loaded revision in that case, so an
/// identity-only condition would overwrite the new text with a merge built from
/// the old text and report success. Exact source contents are part of the
/// condition too: this save is refused, and the ordinary retry then reapplies
/// the edit on top of what the other editor wrote.
#[test]
fn an_in_place_change_after_the_last_check_is_refused_and_reapplied() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    const REPLACEMENT: &str = "\
# Another editor rewrote this same file in place.\n\
[ui]\n\
show_status_bar = false\n";
    let path = temp.path().join("config.toml");
    fs::write(&path, ORIGINAL).expect("the fixture this test named");

    let document = ConfigDocument::load_from_path(&path).expect("load");
    let checked = inode_of(&path);
    let mut edited = document.config().clone();
    edited.presets.set_slot(1, Some(preset("Never written")));

    let error = document
        .save_with_backup_racing_the_write(edited, &mut || {
            fs::write(&path, REPLACEMENT).expect("the in-place edit this test stages");
        })
        .expect_err("changed contents are a stale source even on the same file");

    assert_eq!(
        inode_of(&path),
        checked,
        "the edit must really have kept the same file identity"
    );
    assert!(
        is_stale_source_error(&error),
        "the editors' retry gate must recognise this: {error:#}"
    );
    assert_eq!(
        fs::read_to_string(&path).expect("readable"),
        REPLACEMENT,
        "the other editor's in-place text must not be overwritten"
    );

    persist_preset_slot_at(&path, 1, Some(&preset("Reapplied")))
        .expect("an in-place change is a reload, not a failure");
    let written = fs::read_to_string(&path).expect("readable");
    assert!(
        written.contains("[presets.slot_1]"),
        "the edit must land after reloading the new source: {written}"
    );
    assert!(
        written.contains("Another editor rewrote this same file in place"),
        "and preserve the source it was reapplied onto: {written}"
    );
}

/// The same trick on a config that does not exist yet.
///
/// A load that found nothing expects to find nothing. The save is a creation,
/// so a file that appeared at the name in the meantime is somebody else's first
/// write — their whole file, with nothing of it in the text this save merged.
/// Refusing is what turns it into the reload the editors already know how to
/// do; overwriting would spend their file to say this one succeeded.
#[test]
fn a_file_created_under_a_missing_name_during_the_window_is_refused() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    const FIRST_WRITE: &str = "# Somebody else's first config, written just now.\n";
    let path = temp.path().join("fresh/config.toml");

    let document = ConfigDocument::load_from_path(&path).expect("a missing config loads defaults");
    let mut created = document.config().clone();
    created.presets.set_slot(1, Some(preset("Never written")));

    let mut arrived = false;
    let error = document
        .save_with_backup_racing_the_write(created, &mut || {
            fs::write(&path, FIRST_WRITE).expect("the arrival this test stages");
            arrived = true;
        })
        .expect_err("the name this save was going to create is taken");

    assert!(arrived, "the window must have been raced at all");
    assert!(
        is_stale_source_error(&error),
        "a name that filled up is a reload like any other: {error:#}"
    );
    assert_eq!(
        fs::read_to_string(&path).expect("readable"),
        FIRST_WRITE,
        "the file that got there first keeps every byte"
    );

    persist_preset_slot_at(&path, 1, Some(&preset("Reapplied")))
        .expect("a name that filled up is a reload, not a failure");

    let written = fs::read_to_string(&path).expect("readable");
    assert!(
        written.contains("[presets.slot_1]"),
        "the edit must be reapplied onto the file that got there first: {written}"
    );
    assert!(
        written.contains("Somebody else's first config"),
        "onto what that file already held, not over it: {written}"
    );
}

/// A retarget the save *can* still see is a stale source, not a smaller kind of
/// change.
///
/// The replacement holds byte-identical contents, so nothing about the bytes is
/// different — only the file is. That is enough: the lock this save holds is the
/// other file's, and the document merged its edit into text this path no longer
/// names. Both files must come out untouched, and the error must be the one the
/// editors' reload-and-reapply retry recognises.
#[test]
fn a_retarget_to_identical_bytes_is_still_a_stale_source() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let fixture = linked(temp.path(), ORIGINAL);

    let document = ConfigDocument::load_from_path(&fixture.link).expect("load through the link");
    let mut edited = document.config().clone();
    edited.presets.set_slot(1, Some(preset("Never written")));

    retarget(&fixture);

    let error = document
        .save_with_backup(edited)
        .expect_err("the path no longer names the file this document read");

    assert!(
        is_stale_source_error(&error),
        "the editors' retry gate must recognise this: {error:#}"
    );
    let message = format!("{error:#}");
    assert!(
        message.contains("now resolves to"),
        "and the report must say what changed, not merely that something did: {message}"
    );
    assert_eq!(
        fs::read_to_string(&fixture.original).expect("readable"),
        ORIGINAL,
        "a refused save leaves the file it loaded alone"
    );
    assert_eq!(
        fs::read_to_string(&fixture.replacement).expect("readable"),
        ORIGINAL,
        "and never touches the one it was retargeted onto"
    );
}

/// The profile switch: a retarget one level *up*, where nothing about the config
/// path's own last component moves.
///
/// `active/` points at one profile directory and `active/config.toml` names a
/// real file in both, so following the final component alone sees a plain file
/// that is still a plain file. The two profiles start byte-identical, so the
/// byte comparison sees no change either. Only the file the path names moves —
/// and a save that pins nothing but the leaf writes the loaded document's edit
/// into the profile the user switched *to*, reports success, and leaves the
/// profile they were editing untouched.
///
/// It is a stale source, for the same reason a retargeted leaf link is: the
/// document merged its edit into text this path no longer names. Both profiles
/// must come out of the refused save exactly as they were, and the retry the
/// editors already run must then land the edit on the profile in force.
#[test]
fn a_retargeted_parent_directory_is_a_stale_source_the_retry_reapplies_through() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let first = temp.path().join("profileA");
    let second = temp.path().join("profileB");
    fs::create_dir_all(&first).expect("the first profile this test named");
    fs::create_dir_all(&second).expect("the second profile this test named");
    fs::write(first.join("config.toml"), ORIGINAL).expect("the profile being edited");
    fs::write(second.join("config.toml"), ORIGINAL).expect("the profile switched to");
    let active = temp.path().join("active");
    symlink(&first, &active).expect("a directory link this test's own directory can hold");
    let config = active.join("config.toml");

    let document = ConfigDocument::load_from_path(&config).expect("load through the directory");
    let mut edited = document.config().clone();
    edited.presets.set_slot(1, Some(preset("Never written")));

    // The switch, made while the editor holds its document.
    fs::remove_file(&active).expect("the link this test created");
    symlink(&second, &active).expect("the profile switch this test stages");

    let error = document
        .save_with_backup(edited)
        .expect_err("the path no longer names the file this document read");

    assert!(
        is_stale_source_error(&error),
        "the editors' retry gate must recognise this: {error:#}"
    );
    assert_eq!(
        fs::read_to_string(first.join("config.toml")).expect("readable"),
        ORIGINAL,
        "a refused save leaves the profile it loaded alone"
    );
    assert_eq!(
        fs::read_to_string(second.join("config.toml")).expect("readable"),
        ORIGINAL,
        "and never writes the profile the switch brought in"
    );
    assert!(
        !second.join("config.toml.lock").exists(),
        "the window was never about the profile switched to, so nothing here \
         may have locked it"
    );

    // The recovery, through the editors' own path: a fresh load names the
    // profile in force, and the edit lands there and nowhere else.
    persist_preset_slot_at(&config, 1, Some(&preset("Reapplied")))
        .expect("a switched profile is a reload, not a failure");

    let written = fs::read_to_string(second.join("config.toml")).expect("readable");
    assert!(
        written.contains("[presets.slot_1]"),
        "the edit must land on the profile the path names now: {written}"
    );
    assert!(
        written.contains("setting_from_a_later_release = 7"),
        "onto what that profile already held, not over it: {written}"
    );
    assert_eq!(
        fs::read_to_string(first.join("config.toml")).expect("readable"),
        ORIGINAL,
        "and the profile the path used to name keeps every byte"
    );
}

/// The benign half: a path with no links on it is written where it was named.
///
/// Resolving the directories is what pins the destination, and it must be
/// invisible when there is nothing to resolve — the edit lands on the path the
/// caller gave, the `.bak` lands beside it, and loading that same path again
/// pins it identically, so an ordinary second save is not reported as a change
/// on disk.
#[test]
fn a_path_with_no_links_on_it_is_written_exactly_where_it_was_named() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let directory = temp.path().join("plain");
    fs::create_dir_all(&directory).expect("the fixture directory this test named");
    let path = directory.join("config.toml");
    fs::write(&path, ORIGINAL).expect("the fixture this test named a directory for");

    let document = ConfigDocument::load_from_path(&path).expect("load");
    let mut edited = document.config().clone();
    edited.presets.set_slot(1, Some(preset("Plain")));
    let outcome = document
        .save_with_backup(edited)
        .expect("a plain path saves");

    let written = fs::read_to_string(&path).expect("readable");
    assert!(
        written.contains("[presets.slot_1]"),
        "the edit lands on the path it was made through: {written}"
    );
    assert!(
        written.contains("setting_from_a_later_release = 7"),
        "and keeps what the file already said: {written}"
    );
    let backup = outcome
        .backup_path()
        .expect("an existing file is copied aside");
    assert_eq!(
        backup.parent(),
        Some(directory.as_path()),
        "the copy belongs beside the path the user knows"
    );
    assert_eq!(
        fs::read_to_string(backup).expect("readable"),
        ORIGINAL,
        "and holds what the file said before the write"
    );

    // A second edit through the same unchanged path is an ordinary save.
    persist_preset_slot_at(&path, 2, Some(&preset("Again")))
        .expect("nothing about this path changed, so nothing may report that it did");
}

/// The revision a save leaves behind has to be the one the next save checks
/// against.
///
/// `after_write` records the text that was written and the file the rename
/// created, rather than taking a fresh look at the path — and both halves are
/// load-bearing now that the destination is compared against that identity *and*
/// those exact bytes. Getting either wrong would make the second Save of an
/// editor that never reloaded — which is exactly what the configurator holds —
/// report the file as changed on disk by nobody, with the first save's own text
/// as the thing that changed it.
#[test]
fn a_second_save_through_the_document_the_first_returned_is_not_a_change_on_disk() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let path = temp.path().join("config.toml");
    fs::write(&path, ORIGINAL).expect("the fixture this test named");

    let document = ConfigDocument::load_from_path(&path).expect("load");
    let mut first = document.config().clone();
    first.presets.set_slot(1, Some(preset("First")));
    let (saved, _) = document
        .save_with_backup(first)
        .expect("the first save")
        .into_parts();

    // The editor still holds the document that save handed back. It never
    // reloaded, and nothing else has touched the file.
    let mut second = saved.config().clone();
    second.presets.set_slot(2, Some(preset("Second")));
    saved
        .save_with_backup(second)
        .expect("nothing changed the file between the two saves");

    let written = fs::read_to_string(&path).expect("readable");
    assert!(
        written.contains("[presets.slot_1]") && written.contains("[presets.slot_2]"),
        "both edits must be in the file the two saves shared: {written}"
    );
    assert!(
        written.contains("setting_from_a_later_release = 7"),
        "and neither may lose what the file already said: {written}"
    );
}

/// The case the fallback is for: the config directory does not exist yet.
///
/// A directory cannot be canonicalized before it is made, and the save makes it
/// — so pinning that resolved only existing directories would name the
/// destination one way at load and another way at the check, and every first
/// save into a fresh `~/.config/wayscriber/` would refuse itself as a change on
/// disk. The deepest ancestor that does exist is what both derivations agree on.
#[test]
fn a_first_save_into_a_directory_that_does_not_exist_yet_is_not_a_change_on_disk() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let path = temp.path().join("not-made-yet/wayscriber/config.toml");

    let document = ConfigDocument::load_from_path(&path).expect("a missing config loads defaults");
    let mut created = document.config().clone();
    created.presets.set_slot(1, Some(preset("First")));
    document
        .save_with_backup(created)
        .expect("the save creates the directory it was told to write in");

    let written = fs::read_to_string(&path).expect("readable");
    assert!(
        written.contains("[presets.slot_1]"),
        "the first save lands where it was named: {written}"
    );
}

/// The same first save, through a path that has no ancestor to fall back to.
///
/// A relative config path whose first component is missing gives the walk
/// nothing at all to canonicalize, and a pin that stopped there would be the
/// caller's own relative text. The save creates the directories; the check that
/// follows finds them and pins through them, absolutely — so the two
/// derivations would name one file two ways, and the save would report its own
/// `mkdir` as somebody else's retarget. The configurator has no retry to
/// recover with: its Save would stay refused until the user reloaded, with
/// nothing on disk having changed.
///
/// Both saves have to go through. The first makes the directories, and the
/// second runs through the document that save handed back — the editor that
/// never reloads, which is exactly what the configurator holds.
#[test]
fn a_relative_path_with_no_existing_ancestor_saves_twice_without_reporting_itself() {
    let root = RelativeRoot::new("relative-pin");
    let path = root.relative.join("wayscriber").join("config.toml");

    let document = ConfigDocument::load_from_path(&path).expect("a missing config loads defaults");
    let mut created = document.config().clone();
    created.presets.set_slot(1, Some(preset("First")));
    let (saved, _) = document
        .save_with_backup(created)
        .expect("the save creates the directories it was told to write in")
        .into_parts();

    let mut second = saved.config().clone();
    second.presets.set_slot(2, Some(preset("Second")));
    saved
        .save_with_backup(second)
        .expect("nothing but this save's own directory creation happened in between");

    let written = fs::read_to_string(&path).expect("readable");
    assert!(
        written.contains("[presets.slot_1]") && written.contains("[presets.slot_2]"),
        "both edits must be in the file the two saves shared: {written}"
    );
    assert_eq!(
        fs::canonicalize(&path).expect("the file the two saves wrote"),
        root.absolute.join("wayscriber").join("config.toml"),
        "and the relative path must be written where it named, under the working directory"
    );
}

/// The recovery, staged the way it happens: the retarget lands while the editor
/// is waiting for another writer's window to close.
///
/// The editor wakes, finds that the path resolves somewhere else, and takes the
/// same reload-and-reapply path a changed-bytes conflict takes — so the edit
/// lands on the file the path names *now*, and the file it used to name keeps
/// every byte it had.
#[test]
fn an_edit_that_waits_for_the_lock_reapplies_onto_the_retargeted_file() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    const REPLACEMENT: &str = "# The file the link points at by the time the editor wakes.\n";
    let fixture = linked(temp.path(), REPLACEMENT);

    // The lock a save takes is the resolved file's, so this is the one the
    // editor will wait on.
    let held = hold_config_write_lock(&fixture.original);

    thread::scope(|scope| {
        let waiting =
            scope.spawn(|| persist_preset_slot_at(&fixture.link, 1, Some(&preset("Reapplied"))));

        // The editor is inside its wait by now; move the link and let it
        // through.
        thread::sleep(HOLD);
        retarget(&fixture);
        drop(held);

        waiting
            .join()
            .expect("the waiting editor must not panic")
            .expect("a retarget is a reload, not a failure");
    });

    let replacement = fs::read_to_string(&fixture.replacement).expect("readable");
    assert!(
        replacement.contains("[presets.slot_1]"),
        "the edit must be reapplied onto the file the path names now: {replacement}"
    );
    assert!(
        replacement.contains("The file the link points at"),
        "onto what that file already held, not over it: {replacement}"
    );
    assert_eq!(
        fs::read_to_string(&fixture.original).expect("readable"),
        ORIGINAL,
        "and the file the path used to name keeps every byte"
    );
}
