//! The bodies an extrude's Join, Cut or Intersect acts on. The engine has always taken
//! `targets`; this is the picker that fills it, labelled for the operation that owns it.

use crate::dialogs::{Kind, apply_commands};
use serde_json::json;
use solidcraft_engine::Sel;

const JOIN: &str = "Bodies to Join";
const CUT: &str = "Objects to Cut";

fn scope_row<'a>(h: &'a crate::scenario::Harness, label: &str) -> &'a crate::selection::SelInput {
    h.app.dialog.as_ref().unwrap().inputs.iter().find(|i| i.label == label).expect("the scope row")
}

/// Two separate bars with a sketched rectangle bridging them.
fn two_bars(h: &mut crate::scenario::Harness) -> u64 {
    h.app.run("solid.box", json!({"length":20,"width":20,"height":10,"body_name":"Left"})).unwrap();
    h.app.run("solid.box", json!({"length":20,"width":20,"height":10,"corner":[30,0,0],"body_name":"Right"})).unwrap();
    h.app.run("sketch.create", json!({"plane":"XY","name":"Bridge"})).unwrap();
    h.app.run("sketch.rectangle.two_point", json!({"p0":[5,5],"p1":[45,15]})).unwrap();
    h.app.run("sketch.finish", json!({})).unwrap();
    h.app.session.doc.find_feature("Bridge").unwrap().id
}

#[test]
fn the_picked_bodies_become_the_targets() {
    let mut h = crate::scenario::Harness::new();
    let sketch = two_bars(&mut h);
    h.app.start("solid.extrude");
    h.frames(3);
    let d = h.app.dialog.as_mut().unwrap();
    d.inputs[0].items = vec![Sel::Profile { sketch, index: 0 }];
    let scope = d.inputs.iter_mut().find(|i| i.label == JOIN).expect("the row is there while bodies exist");
    scope.items = vec![Sel::Body { name: "Right".into() }];
    h.frames(3);
    let commands = apply_commands(&h.app, h.app.dialog.as_ref().unwrap()).unwrap();
    let (command, params) = commands.first().unwrap();
    assert_eq!(command, "solid.extrude");
    assert_eq!(params["operation"], "join", "bodies exist, so Join is the default");
    assert_eq!(params["targets"], json!(["Right"]));
}

#[test]
fn an_empty_scope_leaves_targets_out() {
    let mut h = crate::scenario::Harness::new();
    let sketch = two_bars(&mut h);
    h.app.start("solid.extrude");
    h.frames(3);
    h.app.dialog.as_mut().unwrap().inputs[0].items = vec![Sel::Profile { sketch, index: 0 }];
    h.frames(3);
    let commands = apply_commands(&h.app, h.app.dialog.as_ref().unwrap()).unwrap();
    assert!(commands.first().unwrap().1.get("targets").is_none(), "every solid takes part, as before");
}

/// The label follows the operation, and what was picked survives the rename.
#[test]
fn the_label_follows_the_operation() {
    let mut h = crate::scenario::Harness::new();
    two_bars(&mut h);
    h.app.start("solid.extrude");
    h.frames(3);
    let scope = h.app.dialog.as_mut().unwrap().inputs.iter_mut().find(|i| i.label == JOIN).expect("Join by default");
    scope.items = vec![Sel::Body { name: "Right".into() }];
    if let Kind::Extrude { operation, auto_op, .. } = &mut h.app.dialog.as_mut().unwrap().kind {
        *operation = 2;
        *auto_op = false;
    }
    h.frames(3);
    assert_eq!(scope_row(&h, CUT).items.len(), 1, "a cut keeps the body that was picked");
    assert!(h.app.dialog.as_ref().unwrap().inputs.iter().all(|i| i.label != JOIN), "renamed, not duplicated");
}

#[test]
fn new_body_drops_the_row() {
    let mut h = crate::scenario::Harness::new();
    two_bars(&mut h);
    h.app.start("solid.extrude");
    h.frames(3);
    assert!(h.app.dialog.as_ref().unwrap().inputs.iter().any(|i| i.label == JOIN));
    if let Kind::Extrude { operation, auto_op, .. } = &mut h.app.dialog.as_mut().unwrap().kind {
        *operation = 0;
        *auto_op = false;
    }
    h.frames(3);
    let labels: Vec<&str> = h.app.dialog.as_ref().unwrap().inputs.iter().map(|i| i.label).collect();
    assert_eq!(labels, vec!["Profiles"], "a new body acts on nothing");
}

/// The regression the flag exists for: the scope takes bodies, and a face picked to extrude
/// from would otherwise be turned into its body and land there instead of in Profiles.
#[test]
fn a_pre_selected_face_still_fills_profiles() {
    let mut h = crate::scenario::Harness::new();
    h.app.run("solid.box", json!({"length":30,"width":20,"height":20,"body_name":"Plate"})).unwrap();
    let world = h.app.session.world_state();
    let body = world.body("Plate").unwrap();
    h.app.session.selection = body
        .body
        .faces(0.01)
        .unwrap()
        .into_iter()
        .filter(|face| face.plane_normal.is_some_and(|normal| normal.z > 0.99))
        .map(|face| Sel::Face { body: "Plate".into(), index: face.index, point: face.centroid })
        .collect();
    assert_eq!(h.app.session.selection.len(), 1);
    h.app.start("solid.extrude");
    h.frames(3);
    assert_eq!(h.app.dialog.as_ref().unwrap().inputs[0].items.len(), 1, "the face is the profile");
    assert!(scope_row(&h, JOIN).items.is_empty(), "not the scope");
}

/// Editing a scoped extrude shows its scope, and keeps it when nothing is touched.
#[test]
fn editing_a_scoped_extrude_round_trips_its_targets() {
    let mut h = crate::scenario::Harness::new();
    two_bars(&mut h);
    h.app.run("solid.extrude", json!({"sketch":"Bridge","distance":"10 mm","operation":"join","targets":["Right"],"name":"Rib"})).unwrap();
    h.frames(4);
    h.step(&json!({"call":"ui.editFeature","params":{"feature":"Rib"}})).unwrap();
    h.frames(8);
    let scope: Vec<String> =
        scope_row(&h, JOIN).items.iter().map(|x| if let Sel::Body { name } = x { name.clone() } else { String::new() }).collect();
    assert_eq!(scope, vec!["Right".to_string()], "the stored scope shows in the picker");
    // An edit replays as timeline.redefine, which carries the command's own params.
    let commands = apply_commands(&h.app, h.app.dialog.as_ref().unwrap()).unwrap();
    let (command, params) = commands.first().unwrap();
    assert_eq!(command, "timeline.redefine");
    assert_eq!(params["params"]["targets"], json!(["Right"]));
}

/// End to end: click the row, then click a body in the viewport, and it is the scope.
#[test]
fn a_body_can_be_picked_into_the_scope() {
    let mut h = crate::scenario::Harness::new();
    let sketch = two_bars(&mut h);
    h.app.start("solid.extrude");
    h.frames(3);
    h.app.dialog.as_mut().unwrap().inputs[0].items = vec![Sel::Profile { sketch, index: 0 }];
    h.frames(3);
    h.step(&json!({"widget": "↖ Select", "near": JOIN})).unwrap();
    h.frames(2);
    h.step(&json!({"click": {"world": [40.0, 10.0, 10.0]}})).unwrap();
    h.frames(3);
    let picked: Vec<String> =
        scope_row(&h, JOIN).items.iter().map(|x| if let Sel::Body { name } = x { name.clone() } else { String::new() }).collect();
    assert_eq!(picked, vec!["Right".to_string()], "clicking the right bar scopes the join to it");
    let commands = apply_commands(&h.app, h.app.dialog.as_ref().unwrap()).unwrap();
    assert_eq!(commands.first().unwrap().1["targets"], json!(["Right"]));
}

/// Hovering for a body-only input lights the whole body, not the one face under the pointer.
#[test]
fn the_scope_input_hovers_whole_bodies() {
    let mut h = crate::scenario::Harness::new();
    let sketch = two_bars(&mut h);
    h.app.start("solid.extrude");
    h.frames(3);
    h.app.dialog.as_mut().unwrap().inputs[0].items = vec![Sel::Profile { sketch, index: 0 }];
    h.frames(3);
    h.app.dialog.as_mut().unwrap().active = 0;
    h.frames(1);
    assert!(!crate::viewport::body_only_input_active(&h.app), "Profiles takes profiles and faces, so hover stays fine-grained");
    h.step(&json!({"widget": "↖ Select", "near": JOIN})).unwrap();
    h.frames(2);
    assert!(crate::viewport::body_only_input_active(&h.app), "the scope takes whole bodies only");
}

/// Combine's body inputs get the same whole-body hover.
#[test]
fn combines_body_inputs_hover_whole_bodies_too() {
    let mut h = crate::scenario::Harness::new();
    two_bars(&mut h);
    h.app.start("solid.combine");
    h.frames(3);
    assert!(crate::viewport::body_only_input_active(&h.app), "Target Body takes whole bodies");
}
