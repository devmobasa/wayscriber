use std::ffi::OsStr;

use crate::capture::window_geometry::{
    WindowGeometryBackend, WindowQueryContext, WindowTarget, detect_backend_from_env,
    parse_hyprland_targets, parse_sway_targets,
};
use crate::util::Rect;

fn context(name: &str, rect: Rect) -> WindowQueryContext {
    WindowQueryContext {
        output_name: name.to_owned(),
        output_logical_rect: rect,
    }
}

fn assert_target(target: &WindowTarget, id: &str, title: &str, rect: Rect) {
    assert_eq!(target.id, id);
    assert_eq!(target.title, title);
    assert_eq!(target.logical_rect, rect);
}

#[test]
fn environment_detection_prefers_hyprland_then_sway() {
    assert_eq!(
        detect_backend_from_env(Some(OsStr::new("hypr")), Some(OsStr::new("sway"))),
        Some(WindowGeometryBackend::Hyprland)
    );
    assert_eq!(
        detect_backend_from_env(None, Some(OsStr::new("sway"))),
        Some(WindowGeometryBackend::Sway)
    );
    assert_eq!(detect_backend_from_env(None, None), None);
}

#[test]
fn empty_environment_markers_do_not_advertise_a_provider() {
    assert_eq!(
        detect_backend_from_env(Some(OsStr::new("")), Some(OsStr::new(""))),
        None
    );
}

#[test]
fn hyprland_keeps_visible_current_workspace_windows_and_clips_to_output() {
    let monitors = br#"[
      {"id":2,"name":"DP-1","x":1920,"y":-100,"width":2400,"height":1920,
       "scale":1.5,"transform":0,"activeWorkspace":{"id":4,"name":"dev"}}
    ]"#;
    let clients = br#"[
      {"address":"0xa","title":"Editor","mapped":true,"hidden":false,
       "at":[2000,-50],"size":[1000,700],"monitor":2,
       "workspace":{"id":4,"name":"dev"}},
      {"address":"0xb","title":"Clipped","mapped":true,"hidden":false,
       "at":[3400,0],"size":[300,500],"monitor":2,
       "workspace":{"id":4,"name":"dev"}},
      {"address":"0xc","title":"Other workspace","mapped":true,"hidden":false,
       "at":[2000,0],"size":[100,100],"monitor":2,
       "workspace":{"id":5,"name":"chat"}},
      {"address":"0xd","title":"Hidden","mapped":true,"hidden":true,
       "at":[2000,0],"size":[100,100],"monitor":2,
       "workspace":{"id":4,"name":"dev"}},
      {"address":"0xe","title":"Unmapped","mapped":false,"hidden":false,
       "at":[2000,0],"size":[100,100],"monitor":2,
       "workspace":{"id":4,"name":"dev"}},
      {"address":"0xf","title":"Other output","mapped":true,"hidden":false,
       "at":[0,0],"size":[100,100],"monitor":1,
       "workspace":{"id":4,"name":"dev"}}
    ]"#;
    let targets = parse_hyprland_targets(
        monitors,
        clients,
        &context("DP-1", Rect::new(1920, -100, 1600, 1280).unwrap()),
    )
    .unwrap();

    assert_eq!(targets.len(), 2);
    assert_target(
        &targets[0],
        "0xa",
        "Editor",
        Rect::new(80, 50, 1000, 700).unwrap(),
    );
    assert_target(
        &targets[1],
        "0xb",
        "Clipped",
        Rect::new(1480, 100, 120, 500).unwrap(),
    );
}

#[test]
fn hyprland_converts_rotated_fractional_geometry_with_outward_window_edges() {
    let monitors = br#"[
      {"id":9,"name":"DP-2","x":-1080,"y":200,"width":2160,"height":3840,
       "scale":2.0,"transform":1,"activeWorkspace":{"id":7,"name":"7"}}
    ]"#;
    let clients = br#"[
      {"address":"0x9","title":"Fractional","mapped":true,"hidden":false,
       "at":[-1079.75,200.25],"size":[10.5,20.1],"monitor":9,
       "workspace":{"id":7,"name":"7"}}
    ]"#;
    let targets = parse_hyprland_targets(
        monitors,
        clients,
        &context("DP-2", Rect::new(-1080, 200, 1920, 1080).unwrap()),
    )
    .unwrap();

    assert_eq!(targets.len(), 1);
    assert_target(
        &targets[0],
        "0x9",
        "Fractional",
        Rect::new(0, 0, 11, 21).unwrap(),
    );
}

#[test]
fn hyprland_uses_visible_special_workspace_and_keeps_pinned_windows() {
    let monitors = br#"[
      {"id":1,"name":"eDP-1","x":0,"y":0,"width":1920,"height":1080,
       "scale":1.0,"transform":0,"activeWorkspace":{"id":1,"name":"1"},
       "specialWorkspace":{"id":-99,"name":"special:scratch"}}
    ]"#;
    let clients = br#"[
      {"address":"scratch","title":"Scratch","mapped":true,"hidden":false,
       "at":[10,10],"size":[200,100],"monitor":1,
       "workspace":{"id":-99,"name":"special:scratch"}},
      {"address":"pinned","title":"Pinned","mapped":true,"hidden":false,
       "at":[30,30],"size":[200,100],"monitor":1,"pinned":true,
       "workspace":{"id":1,"name":"1"}},
      {"address":"under","title":"Underlying","mapped":true,"hidden":false,
       "at":[50,50],"size":[200,100],"monitor":1,
       "workspace":{"id":1,"name":"1"}}
    ]"#;
    let targets = parse_hyprland_targets(
        monitors,
        clients,
        &context("eDP-1", Rect::new(0, 0, 1920, 1080).unwrap()),
    )
    .unwrap();

    assert_eq!(
        targets
            .iter()
            .map(|target| target.id.as_str())
            .collect::<Vec<_>>(),
        ["scratch", "pinned"]
    );
}

#[test]
fn hyprland_correlates_exact_origin_with_one_pixel_size_tolerance() {
    let monitors = br#"[
      {"id":1,"name":"DP-1","x":0,"y":0,"width":1920,"height":1080,
       "scale":1.0,"transform":0,"activeWorkspace":{"id":1,"name":"1"}}
    ]"#;
    let clients = br#"[
      {"address":"edge","title":"Edge","mapped":true,"hidden":false,
       "at":[1919,0],"size":[2,100],"monitor":1,
       "workspace":{"id":1,"name":"1"}}
    ]"#;

    let smaller = parse_hyprland_targets(
        monitors,
        clients,
        &context("DP-1", Rect::new(0, 0, 1919, 1080).unwrap()),
    )
    .unwrap();
    assert!(smaller.is_empty(), "only the correlated overlap is safe");
    let larger = parse_hyprland_targets(
        monitors,
        clients,
        &context("DP-1", Rect::new(0, 0, 1921, 1080).unwrap()),
    )
    .unwrap();
    assert_eq!(larger[0].logical_rect, Rect::new(1919, 0, 1, 100).unwrap());

    assert!(
        parse_hyprland_targets(
            monitors,
            clients,
            &context("DP-1", Rect::new(1, 0, 1920, 1080).unwrap()),
        )
        .is_err(),
        "origins must match exactly"
    );
    assert!(
        parse_hyprland_targets(
            monitors,
            clients,
            &context("DP-1", Rect::new(0, 0, 1918, 1080).unwrap()),
        )
        .is_err(),
        "size differences above one pixel are stale"
    );
}

#[test]
fn hyprland_rejects_invalid_monitor_scale() {
    let monitors = br#"[
      {"id":1,"name":"DP-1","x":0,"y":0,"width":1920,"height":1080,
       "scale":0.0,"transform":0,"activeWorkspace":{"id":1,"name":"1"}}
    ]"#;
    assert!(
        parse_hyprland_targets(
            monitors,
            br#"[]"#,
            &context("DP-1", Rect::new(0, 0, 1920, 1080).unwrap()),
        )
        .is_err()
    );
}

#[test]
fn sway_keeps_visible_tiled_and_floating_leaves_and_clips_to_output() {
    let tree = br#"{
      "id":1,"type":"root","name":"root","visible":true,
      "rect":{"x":0,"y":0,"width":3520,"height":1280},
      "nodes":[{
        "id":2,"type":"output","name":"DP-1","visible":true,
        "rect":{"x":1920,"y":-100,"width":1600,"height":1280},
        "nodes":[{
          "id":3,"type":"workspace","name":"dev","visible":true,
          "rect":{"x":1920,"y":-100,"width":1600,"height":1280},
          "nodes":[
            {"id":10,"type":"con","name":"Editor","visible":true,"app_id":"editor",
             "rect":{"x":2000,"y":-50,"width":1000,"height":700}},
            {"id":20,"type":"con","name":"split","visible":true,
             "rect":{"x":3300,"y":0,"width":300,"height":500},
             "nodes":[{"id":11,"type":"con","name":"Terminal","visible":true,"window":44,
               "rect":{"x":3400,"y":0,"width":300,"height":500}}]},
            {"id":21,"type":"con","name":"Hidden","visible":false,"app_id":"hidden",
             "rect":{"x":2000,"y":0,"width":100,"height":100}}
          ],
          "floating_nodes":[{"id":12,"type":"floating_con","name":"Palette","visible":true,
            "pid":55,"rect":{"x":2100,"y":800,"width":400,"height":200}}]
        }]
      }]
    }"#;
    let targets = parse_sway_targets(
        tree,
        &context("DP-1", Rect::new(1920, -100, 1600, 1280).unwrap()),
    )
    .unwrap();

    assert_eq!(targets.len(), 3);
    assert_target(
        &targets[0],
        "10",
        "Editor",
        Rect::new(80, 50, 1000, 700).unwrap(),
    );
    assert_target(
        &targets[1],
        "11",
        "Terminal",
        Rect::new(1480, 100, 120, 500).unwrap(),
    );
    assert_target(
        &targets[2],
        "12",
        "Palette",
        Rect::new(180, 900, 400, 200).unwrap(),
    );
}

#[test]
fn sway_walks_floating_wrappers_and_ignores_invalid_leaves() {
    let tree = br#"{
      "id":1,"type":"root","visible":true,"rect":{"x":-1080,"y":0,"width":1080,"height":900},
      "nodes":[{"id":2,"type":"output","name":"DP-3","visible":true,
        "rect":{"x":-1080,"y":0,"width":1080,"height":900},
        "nodes":[{"id":3,"type":"workspace","name":"4","visible":true,
          "rect":{"x":-1080,"y":0,"width":1080,"height":900},
          "floating_nodes":[{"id":4,"type":"floating_con","visible":true,
            "rect":{"x":-1000,"y":50,"width":500,"height":400},
            "nodes":[
              {"id":5,"type":"con","name":"Dialog","visible":true,"window_properties":{},
               "rect":{"x":-1000,"y":50,"width":500,"height":400}},
              {"id":6,"type":"con","name":"Invalid","visible":true,"app_id":"bad",
               "rect":{"x":-1000,"y":50,"width":0,"height":20}}
            ]}]
        }]
      }]
    }"#;
    let targets = parse_sway_targets(
        tree,
        &context("DP-3", Rect::new(-1080, 0, 1080, 900).unwrap()),
    )
    .unwrap();

    assert_eq!(targets.len(), 1);
    assert_target(
        &targets[0],
        "5",
        "Dialog",
        Rect::new(80, 50, 500, 400).unwrap(),
    );
}

#[test]
fn sway_correlates_exact_origin_with_one_pixel_size_tolerance() {
    let tree = br#"{
      "id":1,"type":"root","visible":true,"rect":{"x":0,"y":0,"width":1920,"height":1080},
      "nodes":[{"id":2,"type":"output","name":"DP-1","visible":true,
        "rect":{"x":0,"y":0,"width":1920,"height":1080},
        "nodes":[{"id":3,"type":"workspace","name":"1","visible":true,
          "rect":{"x":0,"y":0,"width":1920,"height":1080},
          "nodes":[{"id":10,"type":"con","name":"Edge","visible":true,"app_id":"edge",
            "rect":{"x":1919,"y":0,"width":2,"height":100}}]
        }]
      }]
    }"#;

    let smaller =
        parse_sway_targets(tree, &context("DP-1", Rect::new(0, 0, 1919, 1080).unwrap())).unwrap();
    assert!(smaller.is_empty(), "only the correlated overlap is safe");
    let larger =
        parse_sway_targets(tree, &context("DP-1", Rect::new(0, 0, 1921, 1080).unwrap())).unwrap();
    assert_eq!(larger[0].logical_rect, Rect::new(1919, 0, 1, 100).unwrap());

    assert!(
        parse_sway_targets(tree, &context("DP-1", Rect::new(1, 0, 1920, 1080).unwrap()),).is_err(),
        "origins must match exactly"
    );
    assert!(
        parse_sway_targets(tree, &context("DP-1", Rect::new(0, 0, 1918, 1080).unwrap()),).is_err(),
        "size differences above one pixel are stale"
    );
}
