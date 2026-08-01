use super::*;

fn selection_entry(
    label: &str,
    value: &str,
    kind: SelectionPropertyKind,
    disabled: bool,
) -> SelectionPropertyEntry {
    SelectionPropertyEntry {
        label: label.to_string(),
        value: value.to_string(),
        kind,
        disabled,
    }
}

fn selection_snapshot() -> ToolbarSnapshot {
    let mut snapshot = snapshot_for_tool(Tool::Select);
    snapshot.selection_properties = vec![
        selection_entry("Color", "Red", SelectionPropertyKind::Color, false),
        selection_entry(
            "Thickness",
            "3.0px",
            SelectionPropertyKind::Thickness,
            false,
        ),
        selection_entry("Fill", "Off", SelectionPropertyKind::Fill, false),
        selection_entry(
            "Arrow angle",
            "Locked",
            SelectionPropertyKind::ArrowAngle,
            true,
        ),
    ];
    snapshot
}

#[test]
fn select_with_a_selection_docks_the_property_entries_in_order() {
    let snapshot = selection_snapshot();
    assert_eq!(
        StylePillSpec::state_of(&snapshot, &plan()),
        StylePillState::Selection
    );
    let spec = StylePillSpec::build(&snapshot, &plan());
    assert_eq!(spec.state(), StylePillState::Selection);
    assert_eq!(
        control_ids(&spec),
        [
            "top.style.sel.color",
            "top.style.sel.thickness",
            "top.style.sel.fill",
            "top.style.sel.arrow-angle",
        ]
    );
    assert_eq!(
        spec.controls()[0],
        StylePillControl::SelectionCycle(SelectionPropertyKind::Color)
    );
    assert_eq!(
        spec.controls()[1],
        StylePillControl::SelectionStepper(SelectionPropertyKind::Thickness)
    );

    // Select without a selection stays hidden.
    let empty = snapshot_for_tool(Tool::Select);
    assert_eq!(
        StylePillSpec::state_of(&empty, &plan()),
        StylePillState::Hidden
    );
    assert!(StylePillSpec::build(&empty, &plan()).controls().is_empty());
}

#[test]
fn selection_cycles_step_forward_through_the_apply_machinery() {
    let snapshot = selection_snapshot();
    let cycle = StylePillControl::SelectionCycle(SelectionPropertyKind::Color);
    assert_eq!(cycle.role(), StylePillRole::Button);
    assert!(cycle.enabled(&snapshot));
    assert_eq!(
        cycle.event(&snapshot),
        Some(ToolbarEvent::AdjustSelectionProperty {
            kind: SelectionPropertyKind::Color,
            direction: 1,
        })
    );
    assert_eq!(cycle.value_text(&snapshot).as_deref(), Some("Red"));
    assert_eq!(cycle.label(&snapshot).as_ref(), "Color");
    assert_eq!(cycle.tooltip(&snapshot).as_deref(), Some("Color: Red"));
    assert_eq!(cycle.steps(&snapshot), None);
}

#[test]
fn selection_steppers_carry_minus_plus_halves() {
    let snapshot = selection_snapshot();
    let stepper = StylePillControl::SelectionStepper(SelectionPropertyKind::Thickness);
    assert_eq!(stepper.role(), StylePillRole::Stepper);
    assert!(stepper.enabled(&snapshot));
    assert_eq!(stepper.event(&snapshot), None, "halves carry the events");
    assert_eq!(stepper.value_text(&snapshot).as_deref(), Some("3.0px"));

    let steps = stepper.steps(&snapshot).expect("stepper halves");
    assert_eq!(steps[0].id, "top.style.sel.thickness.minus");
    assert_eq!(steps[1].id, "top.style.sel.thickness.plus");
    assert_eq!(
        steps[0].event,
        ToolbarEvent::AdjustSelectionProperty {
            kind: SelectionPropertyKind::Thickness,
            direction: -1,
        }
    );
    assert_eq!(
        steps[1].event,
        ToolbarEvent::AdjustSelectionProperty {
            kind: SelectionPropertyKind::Thickness,
            direction: 1,
        }
    );
    assert_eq!(steps[0].tooltip, "Decrease thickness");
    assert_eq!(steps[1].tooltip, "Increase thickness");
}

#[test]
fn locked_selection_entries_disable_their_controls() {
    let snapshot = selection_snapshot();
    let locked = StylePillControl::SelectionStepper(SelectionPropertyKind::ArrowAngle);
    assert!(!locked.enabled(&snapshot));
    assert_eq!(locked.value_text(&snapshot).as_deref(), Some("Locked"));
    // An entry the selection does not expose is disabled too.
    let missing = StylePillControl::SelectionCycle(SelectionPropertyKind::TextBackground);
    assert!(!missing.enabled(&snapshot));
    assert_eq!(missing.value_text(&snapshot), None);
}

#[test]
fn ids_are_stable_and_unique_per_spec() {
    for tool in [
        Tool::Pen,
        Tool::Marker,
        Tool::Eraser,
        Tool::Rect,
        Tool::Arrow,
        Tool::StepMarker,
    ] {
        let snapshot = snapshot_for_tool(tool);
        let ids = control_ids(&StylePillSpec::build(&snapshot, &plan()));
        let mut sorted = ids.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len(), "{tool:?} ids unique: {ids:?}");
        for id in &ids {
            assert!(id.starts_with("top.style."), "{id} uses the pill prefix");
        }
    }

    // Classic mode (context_aware_ui = false) can materialize BOTH
    // counter resets in one spec: the step marker's plus the arrow
    // counter's (arrow auto-numbering enabled). Their ids must stay
    // distinct so focus/updater resolution by id is unambiguous.
    let mut classic = snapshot_for_tool(Tool::StepMarker);
    classic.context_aware_ui = false;
    classic.arrow_label_enabled = true;
    let ids = control_ids(&StylePillSpec::build(&classic, &plan()));
    assert!(
        ids.contains(&"top.style.counter-reset.arrow".to_string()),
        "classic ids: {ids:?}"
    );
    assert!(
        ids.contains(&"top.style.counter-reset.step".to_string()),
        "classic ids: {ids:?}"
    );
    let mut sorted = ids.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), ids.len(), "classic-mode ids unique: {ids:?}");
}

#[test]
fn allocation_free_queries_match_the_materialized_spec() {
    let mut minimized = snapshot();
    minimized.top_minimized = true;
    let mut micro = snapshot();
    micro.top_display_mode = crate::config::TopDisplayMode::Micro;
    let mut text = snapshot();
    text.text_active = true;

    let mut cases = vec![minimized, micro, text];
    for tool in [
        Tool::Select,
        Tool::Pen,
        Tool::Marker,
        Tool::Eraser,
        Tool::Rect,
        Tool::Arrow,
        Tool::StepMarker,
    ] {
        cases.push(snapshot_for_tool(tool));
    }

    for snapshot in cases {
        let spec = StylePillSpec::build(&snapshot, &plan());
        assert_eq!(StylePillSpec::state_of(&snapshot, &plan()), spec.state());
        assert_eq!(
            StylePillSpec::visible(&snapshot, &plan()),
            !spec.controls().is_empty(),
            "visible() must equal a non-empty control list ({:?})",
            spec.state()
        );
    }
}
